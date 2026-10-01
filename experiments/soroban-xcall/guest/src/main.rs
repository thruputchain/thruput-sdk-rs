//! Experiment: Soroban contracts as Groundhog contracts, with cross-contract
//! calls in every direction.
//!
//! Each Groundhog contract built from this crate hosts ONE natively compiled
//! Soroban contract (picked by cargo feature) plus soroban-env-host, all RV32.
//!
//! - Identity: the Soroban contract id IS this Groundhog contract's address,
//!   so both worlds name a contract by the same 32 bytes.
//! - Method 0: calldata is an XDR `HostFunction::InvokeContract` targeting
//!   this contract; returns the XDR `ScVal` result. A Soroban error aborts the
//!   Groundhog frame (reverting it) with an XDR `ScVal::Error` payload; a
//!   panic traps the frame. Either way the caller's frame survives -- this is
//!   what replaces the Wasm VM's trap boundary.
//! - Outbound calls: any Soroban call to a *different* contract id leaves this
//!   host and becomes a Groundhog `Invoke` of that address: method 0 with an
//!   XDR `HostFunction` (Soroban -> Soroban), or, for the reserved function
//!   name `gh_raw(method: u64, calldata: Bytes) -> Bytes`, a raw call of any
//!   Groundhog contract (Soroban -> Groundhog).
//! - Storage: Soroban ledger entries live in this contract's own Groundhog
//!   storage (the storage shim, as in ../soroban-native).
//! - Auth: Soroban auth trees arrive as Groundhog auth tokens minted by the
//!   trusted Soroban entrypoint (`gh-entry`), which has already checked the
//!   signatures. See the `auth` module.

#![no_std]
#![no_main]

extern crate alloc;
extern crate std;

mod atomics;

use alloc::{boxed::Box, rc::Rc, vec::Vec};
use sdk::types::StorageKey;
use soroban_env_host::{
    budget::Budget,
    e2e_invoke::{
        RecordingInvocationAuthMode,
        invoke_host_function_in_recording_mode_with_setup,
    },
    storage::{EntryWithLiveUntil, SnapshotSource},
    xdr::{
        AccountId, ContractDataDurability, ContractExecutable, ContractId, Hash, HostFunction,
        LedgerEntry, LedgerEntryData, LedgerEntryExt, LedgerKey, Limits, PublicKey, ReadXdr,
        ScAddress, ScContractInstance, ScError, ScErrorCode, ScErrorType, ScVal, Uint256,
        WriteXdr,
    },
    Host, HostError, LedgerInfo,
};

// ---------------------------------------------------------------- platform --

// A real free-list allocator over the SDK linker script's heap span. std's
// zkvm `System` allocator never frees, and Soroban churns allocations.
#[global_allocator]
static ALLOC: talc::Talck<spin::Mutex<()>, talc::ClaimOnOom> =
    talc::Talc::new(unsafe { talc::ClaimOnOom::new(talc::Span::empty()) }).lock();

unsafe extern "C" {
    static mut __heap_base: u8;
    static mut __heap_end: u8;
}

fn init_heap() {
    unsafe {
        let base = &raw mut __heap_base;
        let end = &raw mut __heap_end;
        ALLOC.lock().claim(talc::Span::new(base, end)).ok();
    }
}

// Deterministic contract: no entropy. Soroban seeds its own PRNG explicitly.
fn no_entropy(_: &mut [u8]) -> Result<(), getrandom::Error> {
    Err(getrandom::Error::UNSUPPORTED)
}
getrandom::register_custom_getrandom!(no_entropy);

// The handful of platform hooks std's zkvm PAL links against.
mod zkvm_pal {
    fn trap() -> ! {
        unsafe { core::arch::asm!("unimp", options(noreturn)) }
    }
    /// A panic can't be unwound here, so it ends the Groundhog frame. Revert
    /// it with the Soroban error that caused the panic (a host error the
    /// contract didn't handle), or -- for a plain contract panic -- with the
    /// error a trapped Wasm contract produces. The caller sees exactly what a
    /// failed Wasm callee would give it.
    #[unsafe(no_mangle)]
    extern "C" fn sys_panic(ptr: *const u8, len: usize) -> ! {
        sdk::log::log(unsafe { core::slice::from_raw_parts(ptr, len) });
        super::abort_with(soroban_env_host::take_last_escalated_error().unwrap_or_else(super::trapped))
    }
    #[unsafe(no_mangle)]
    extern "C" fn sys_halt() -> ! {
        trap()
    }
    #[unsafe(no_mangle)]
    extern "C" fn sys_log(ptr: *const u8, len: usize) {
        sdk::log::log(unsafe { core::slice::from_raw_parts(ptr, len) });
    }
    #[unsafe(no_mangle)]
    extern "C" fn sys_write(_fd: u32, ptr: *const u8, len: usize) {
        sdk::log::log(unsafe { core::slice::from_raw_parts(ptr, len) });
    }
    #[unsafe(no_mangle)]
    extern "C" fn sys_read(_fd: u32, _buf: *mut u8, _n: usize) -> usize {
        0
    }
    #[unsafe(no_mangle)]
    extern "C" fn sys_rand(buf: *mut u32, words: usize) {
        // Only std's HashMap RandomState asks for this. A contract must be
        // deterministic, so hand out fixed keys (HashDoS resistance is moot:
        // execution is gas-metered).
        for i in 0..words {
            unsafe { buf.add(i).write(0x9e37_79b9u32.wrapping_mul(i as u32 + 1)) };
        }
    }
    #[unsafe(no_mangle)]
    extern "C" fn sys_getenv(_b: *mut u32, _w: usize, _k: *const u8, _l: usize) -> usize {
        usize::MAX
    }
    #[unsafe(no_mangle)]
    extern "C" fn sys_argc() -> usize {
        0
    }
    #[unsafe(no_mangle)]
    extern "C" fn sys_argv(_o: *mut u32, _n: usize, _i: usize) -> usize {
        0
    }
    #[unsafe(no_mangle)]
    extern "C" fn sys_cycle_count() -> usize {
        0
    }
    #[unsafe(no_mangle)]
    extern "C" fn sys_alloc_aligned(_bytes: usize, _align: usize) -> *mut u8 {
        trap() // unused: #[global_allocator] above replaces std's System.
    }
}

// ----------------------------------------------------------- storage shim --

const CHUNK: usize = 1024;
const HDR: usize = 8;
const NO_TTL: u32 = u32::MAX;

// Every first touch of a Groundhog storage key costs a DB read (37,300 gas),
// so the layout minimises keys: chunk 0 lives at the base key and carries the
// header, so entries up to CHUNK - HDR bytes take exactly one key.
//   base key = sha256("sorhog" || LedgerKey XDR)
//   chunk 0  @ base key         = len:u32 || live_until:u32 || data[..CHUNK-HDR]
//   chunk i  @ sha256(base || i) = next CHUNK bytes (i >= 1)

fn base_key(ledger_key_xdr: &[u8]) -> [u8; 32] {
    let mut buf = Vec::with_capacity(6 + ledger_key_xdr.len());
    buf.extend_from_slice(b"sorhog");
    buf.extend_from_slice(ledger_key_xdr);
    sdk::crypto::sha256(&buf).into_inner()
}

fn chunk_key(base: &[u8; 32], i: u32) -> StorageKey {
    if i == 0 {
        return (*base).into();
    }
    let mut buf = [0u8; 36];
    buf[..32].copy_from_slice(base);
    buf[32..].copy_from_slice(&i.to_le_bytes());
    sdk::crypto::sha256(&buf).into_inner().into()
}

/// Number of chunks (keys) an entry of `len` bytes occupies.
fn chunk_count(len: usize) -> usize {
    1 + (len + HDR).saturating_sub(CHUNK).div_ceil(CHUNK)
}

/// Reads chunk 0; returns (len, live_until, first data bytes).
fn read_first(base: &[u8; 32]) -> Option<(usize, u32, Box<[u8]>)> {
    let first = sdk::bytestring_try_get(&chunk_key(base, 0)).ok()?;
    let len = u32::from_le_bytes(first[..4].try_into().unwrap()) as usize;
    let live_until = u32::from_le_bytes(first[4..8].try_into().unwrap());
    Some((len, live_until, first))
}

fn read_entry(ledger_key_xdr: &[u8]) -> Option<(Vec<u8>, u32)> {
    let base = base_key(ledger_key_xdr);
    let (len, live_until, first) = read_first(&base)?;
    let mut out = Vec::with_capacity(len);
    out.extend_from_slice(&first[HDR..]);
    let mut i = 1;
    while out.len() < len {
        out.extend_from_slice(&sdk::bytestring_get(&chunk_key(&base, i)));
        i += 1;
    }
    Some((out, live_until))
}

fn write_entry(ledger_key_xdr: &[u8], value: &[u8], live_until: u32) {
    let base = base_key(ledger_key_xdr);
    let old_chunks = read_first(&base).map_or(0, |(len, _, _)| chunk_count(len));
    let split = value.len().min(CHUNK - HDR);
    let mut first = Vec::with_capacity(HDR + split);
    first.extend_from_slice(&(value.len() as u32).to_le_bytes());
    first.extend_from_slice(&live_until.to_le_bytes());
    first.extend_from_slice(&value[..split]);
    sdk::bytestring_set(&chunk_key(&base, 0), &first);
    let mut n = 1;
    for (i, c) in value[split..].chunks(CHUNK).enumerate() {
        sdk::bytestring_set(&chunk_key(&base, i as u32 + 1), c);
        n = i + 2;
    }
    for i in n..old_chunks {
        sdk::delete_shared(&chunk_key(&base, i as u32));
    }
}

fn delete_entry(ledger_key_xdr: &[u8]) {
    let base = base_key(ledger_key_xdr);
    if let Some((len, _, _)) = read_first(&base) {
        for i in 0..chunk_count(len) {
            sdk::delete_shared(&chunk_key(&base, i as u32));
        }
    }
}


/// The shim: Soroban asks for ledger entries, we answer from Groundhog storage.
///
/// Answers are memoised per transaction (Soroban asks for the same keys
/// several times). Two entries are synthesised rather than stored: the code
/// entry of the compiled-in contract, and -- until the contract first writes
/// it -- this contract's instance entry, which is what makes "the Groundhog
/// address is the Soroban contract id" work without a create step.
struct HogSnapshot {
    me: ContractId,
    memo: core::cell::RefCell<alloc::collections::BTreeMap<Vec<u8>, Option<EntryWithLiveUntil>>>,
}

impl SnapshotSource for HogSnapshot {
    fn get(&self, key: &Rc<LedgerKey>) -> Result<Option<EntryWithLiveUntil>, HostError> {
        let kx = key.to_xdr(Limits::none())?;
        if let Some(hit) = self.memo.borrow().get(&kx) {
            return Ok(hit.clone());
        }
        let stored = || {
            read_entry(&kx).map(|(bytes, live_until)| {
                let entry = LedgerEntry::from_xdr(bytes, Limits::none()).unwrap();
                let ttl = (live_until != NO_TTL).then_some(live_until);
                (Rc::new(entry), ttl)
            })
        };
        let r = match key.as_ref() {
            LedgerKey::ContractCode(cc) if cc.hash.0 == native::hash() => {
                Some((Rc::new(native::code_stub()), Some(u32::MAX)))
            }
            LedgerKey::ContractData(cd)
                if cd.contract == ScAddress::Contract(self.me.clone())
                    && cd.key == ScVal::LedgerKeyContractInstance
                    && cd.durability == ContractDataDurability::Persistent =>
            {
                stored().or_else(|| Some((Rc::new(default_instance(&self.me)), Some(u32::MAX))))
            }
            _ => stored(),
        };
        self.memo.borrow_mut().insert(kx, r.clone());
        Ok(r)
    }
}

fn default_instance(me: &ContractId) -> LedgerEntry {
    use soroban_env_host::xdr::ContractDataEntry;
    LedgerEntry {
        last_modified_ledger_seq: 0,
        data: LedgerEntryData::ContractData(ContractDataEntry {
            ext: soroban_env_host::xdr::ExtensionPoint::V0,
            contract: ScAddress::Contract(me.clone()),
            key: ScVal::LedgerKeyContractInstance,
            durability: ContractDataDurability::Persistent,
            val: ScVal::ContractInstance(ScContractInstance {
                executable: ContractExecutable::Wasm(Hash(native::hash())),
                storage: None,
            }),
        }),
        ext: LedgerEntryExt::V0,
    }
}

// ------------------------------------------------------- native contracts --

mod native {
    use super::*;
    use soroban_env_host::{
        ContractFunctionSet, EnvBase as _, Symbol, SymbolStr, TryFromVal, Val,
    };

    /// Builds a `ContractFunctionSet` dispatching function names to a
    /// `#[contractimpl]`'s generated native entrypoints.
    macro_rules! native_contract {
        ($name:ident, $label:literal, $krate:ident :: $ty:ident {
            $($func:ident => $raw:ident ($($arg:tt),*)),* $(,)?
        }) => {
            pub const LABEL: &str = $label;
            pub struct $name;
            impl ContractFunctionSet for $name {
                #[allow(deprecated)]
                fn call(&self, func: &Symbol, host: &Host, args: &[Val]) -> Option<Val> {
                    let sym = SymbolStr::try_from_val(host, func).ok()?;
                    let name: &str = sym.as_ref();
                    let env = soroban_sdk::Env::__from_host(host.clone());
                    $(
                        if name == stringify!($func) {
                            let n: usize = 0 $(+ { let _ = $arg; 1 })*;
                            if args.len() != n {
                                return None;
                            }
                            return Some($krate::$raw(env, $(args[$arg]),*));
                        }
                    )*
                    None
                }
            }
            pub fn fns() -> Rc<dyn ContractFunctionSet> {
                Rc::new($name)
            }
        };
    }

    #[cfg(feature = "counter")]
    native_contract!(Fns, "soroban-counter", soroban_counter::Counter {
        increment => __Counter__increment__invoke_raw(0),
        get => __Counter__get__invoke_raw(),
        inc_then_panic => __Counter__inc_then_panic__invoke_raw(0),
        inc_then_err => __Counter__inc_then_err__invoke_raw(0),
    });

    #[cfg(feature = "caller")]
    native_contract!(Fns, "soroban-caller", soroban_caller::Caller {
        call_inc => __Caller__call_inc__invoke_raw(0, 1),
        try_panic => __Caller__try_panic__invoke_raw(0, 1),
        try_err => __Caller__try_err__invoke_raw(0, 1),
        gh_call => __Caller__gh_call__invoke_raw(0, 1, 2),
    });

    #[cfg(feature = "token")]
    native_contract!(Fns, "soroban-token", soroban_token::Token {
        mint => __Token__mint__invoke_raw(0, 1),
        balance => __Token__balance__invoke_raw(0),
        transfer => __Token__transfer__invoke_raw(0, 1, 2),
    });

    #[cfg(feature = "router")]
    native_contract!(Fns, "soroban-router", soroban_router::Router {
        pay => __Router__pay__invoke_raw(0, 1, 2, 3),
        pay_evil => __Router__pay_evil__invoke_raw(0, 1, 2, 3),
        pay_twice => __Router__pay_twice__invoke_raw(0, 1, 2, 3),
        pay_self => __Router__pay_self__invoke_raw(0, 1, 2),
    });

    /// Pseudo Wasm hash the compiled-in contract is registered under.
    pub fn hash() -> [u8; 32] {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"sorhog-native:");
        buf.extend_from_slice(LABEL.as_bytes());
        sdk::crypto::sha256(&buf).into_inner()
    }

    /// The code entry the host expects for a native contract: no Wasm bytes.
    pub fn code_stub() -> LedgerEntry {
        use soroban_env_host::xdr::{ContractCodeEntry, ContractCodeEntryExt};
        LedgerEntry {
            last_modified_ledger_seq: 0,
            data: LedgerEntryData::ContractCode(ContractCodeEntry {
                ext: ContractCodeEntryExt::V0,
                hash: Hash(hash()),
                code: Default::default(),
            }),
            ext: LedgerEntryExt::V0,
        }
    }

    pub fn register(host: &Host) -> Result<(), HostError> {
        let h = host.bytes_new_from_slice(&hash())?;
        host.register_native_contract_as_wasm(fns(), h)
    }
}

fn is_native_code_key(key_xdr: &[u8]) -> bool {
    matches!(LedgerKey::from_xdr(key_xdr, Limits::none()),
        Ok(LedgerKey::ContractCode(cc)) if cc.hash.0 == native::hash())
}

// ------------------------------------------------------------------- auth --

/// Soroban auth trees, carried as Groundhog auth tokens.
///
/// The trusted Soroban entrypoint verifies signatures, then mints one token
/// tree per Soroban authorization entry. Node layout:
///   consumer_address = the Soroban contract the node authorizes
///   payload          = XDR ScVal::Vec[authorizer: Address, fn: Symbol,
///                                     args: Vec, child_fps: Bytes]
///   child_nodes      = the node's sub-invocations
/// `child_fps` holds an 8-byte fingerprint per child (see [`fingerprint`]):
/// only a token's consumer can open it, so a parent routes its children by
/// fingerprint without seeing them.
///
/// What the Groundhog runtime guarantees, and why it suffices against an
/// untrusted contract anywhere in the call chain:
/// - the minter is stamped by the runtime: only the entrypoint can mint
///   tokens we accept (checked below), so auth can't be forged;
/// - only the named consumer can open a node, once: a child can't be redirected
///   to another contract or reused;
/// - the payload pins the exact call: the host enforces the tree against the
///   real call, so altered arguments fail `require_auth`;
/// - forwarded tokens return to the caller if the callee fails.
mod auth {
    use super::*;
    use soroban_env_host::xdr::{
        InvokeContractArgs, ScSymbol, SorobanAuthorizedFunction, SorobanAuthorizedInvocation,
    };

    /// 8-byte fingerprint of a call: sha256(XDR InvokeContractArgs)[..8].
    pub fn fingerprint(contract: &ContractId, func: &ScSymbol, args: &[ScVal]) -> [u8; 8] {
        let ica = InvokeContractArgs {
            contract_address: ScAddress::Contract(contract.clone()),
            function_name: func.clone(),
            args: args.to_vec().try_into().unwrap(),
        };
        let h = sdk::crypto::sha256(&ica.to_xdr(Limits::none()).unwrap()).into_inner();
        h[..8].try_into().unwrap()
    }

    fn entrypoint_key() -> StorageKey {
        sdk::crypto::sha256(b"sorhog:trusted-entrypoint").into_inner().into()
    }

    /// The Soroban entrypoint whose tokens this contract accepts. Set once
    /// (method 2) right after deployment; a real deployment would fix it at
    /// build time or genesis instead.
    pub fn trusted_entrypoint() -> Option<[u8; 32]> {
        sdk::bytestring_exists(&entrypoint_key())
            .then(|| sdk::bytestring_get_sized::<32>(&entrypoint_key()))
    }

    pub fn set_trusted_entrypoint(addr: &[u8]) {
        if trusted_entrypoint().is_some() || addr.len() != 32 {
            sdk::contract_op::contract_revert();
        }
        sdk::bytestring_set(&entrypoint_key(), addr);
    }

    /// A child token this contract holds, routable by call fingerprint.
    pub struct Held {
        pub id: u32,
        pub fp: [u8; 8],
        pub used: core::cell::Cell<bool>,
    }

    /// Consume the root tokens passed with this invocation. Returns the
    /// pre-verified auth entries for the host plus the child tokens to route.
    pub fn consume_roots(
        me: &ContractId,
        token_ids: &[u32],
    ) -> Result<(Vec<(ScAddress, SorobanAuthorizedInvocation)>, Vec<Held>), soroban_env_host::Error>
    {
        let bad = || soroban_env_host::Error::from_type_and_code(ScErrorType::Auth, ScErrorCode::InvalidInput);
        let mut entries = Vec::new();
        let mut held = Vec::new();
        if token_ids.is_empty() {
            return Ok((entries, held));
        }
        let trusted = trusted_entrypoint().ok_or_else(bad)?;
        for &id in token_ids {
            let (payload, minter, child_start, child_count) = consume(id).ok_or_else(bad)?;
            if minter != trusted {
                return Err(bad()); // forged: not minted by the trusted entrypoint
            }
            let ScVal::Vec(Some(parts)) = ScVal::from_xdr(payload, Limits::none()).map_err(|_| bad())? else {
                return Err(bad());
            };
            let [ScVal::Address(authorizer), ScVal::Symbol(func), ScVal::Vec(args), ScVal::Bytes(fps)] =
                parts.as_slice()
            else {
                return Err(bad());
            };
            if fps.len() != 8 * child_count as usize {
                return Err(bad());
            }
            let args = args.clone().unwrap_or_default();
            entries.push((
                authorizer.clone(),
                SorobanAuthorizedInvocation {
                    function: SorobanAuthorizedFunction::ContractFn(InvokeContractArgs {
                        contract_address: ScAddress::Contract(me.clone()),
                        function_name: func.clone(),
                        args: args.to_vec().try_into().map_err(|_| bad())?,
                    }),
                    // Sub-invocations run in other Groundhog contracts; they
                    // travel as the child tokens below.
                    sub_invocations: Default::default(),
                },
            ));
            for (i, fp) in fps.as_slice().chunks(8).enumerate() {
                held.push(Held {
                    id: child_start + i as u32,
                    fp: fp.try_into().unwrap(),
                    used: core::cell::Cell::new(false),
                });
            }
        }
        Ok((entries, held))
    }

    /// ConsumeAuthToken: (payload, minter, first child id, child count).
    fn consume(id: u32) -> Option<(Vec<u8>, [u8; 32], u32, u32)> {
        use core::mem::MaybeUninit;
        let mut buf = alloc::vec![MaybeUninit::<u8>::uninit(); 512];
        let mut written = MaybeUninit::uninit();
        let mut child_start = MaybeUninit::uninit();
        let mut child_count = MaybeUninit::uninit();
        let mut minter = MaybeUninit::uninit();
        sdk::sys::uninit_ConsumeAuthToken(
            id,
            &mut buf[..],
            &mut minter,
            &mut written,
            &mut child_start,
            &mut child_count,
        )
        .ok()?;
        unsafe {
            let n = written.assume_init() as usize;
            let payload = buf[..n].iter().map(|b| b.assume_init()).collect();
            let minter: sdk::types::ContractAddress = minter.assume_init();
            Some((payload, minter.into_inner(), child_start.assume_init(), child_count.assume_init()))
        }
    }
}

// ----------------------------------------------------------- call routing --

/// Routes Soroban calls that leave this contract to Groundhog invocations.
struct GroundhogRouter {
    me: ContractId,
    /// Child auth tokens from consumed roots, routed to matching calls.
    held: Vec<auth::Held>,
}

/// What a caller sees when the callee's frame trapped (a native panic): the
/// same error a trapped Wasm callee produces.
fn trapped() -> soroban_env_host::Error {
    soroban_env_host::Error::from_type_and_code(ScErrorType::WasmVm, ScErrorCode::InvalidAction)
}

fn gh_invoke(
    target: &ContractId,
    method: u64,
    calldata: &[u8],
    tokens: &[u32],
) -> Result<Vec<u8>, Vec<u8>> {
    use sdk::call::{contract_call, CallOutcome};
    let mut ret = Vec::new();
    let mut ret_auth = Vec::new();
    let outcome = contract_call(
        &sdk::types::ContractAddress::from(target.0 .0),
        sdk::contract::ContractEntrypointID::from_u64(method),
        u64::MAX,
        calldata,
        &tokens.to_vec(),
        Default::default(),
        &mut ret,
        &mut ret_auth,
    );
    match outcome {
        Ok(CallOutcome::Return) => Ok(ret),
        Ok(CallOutcome::Abort) => Err(ret),
        Err(_) => Err(Vec::new()), // trap, re-entrance, depth, ...
    }
}

impl soroban_env_host::ExternalContractHandler for GroundhogRouter {
    fn is_external(&self, id: &ContractId) -> bool {
        *id != self.me
    }

    fn call(
        &self,
        id: &ContractId,
        func: &soroban_env_host::xdr::ScSymbol,
        args: Vec<ScVal>,
    ) -> Result<ScVal, soroban_env_host::Error> {
        let fail = || soroban_env_host::Error::from_type_and_code(
            ScErrorType::Context, ScErrorCode::InvalidAction);
        if func.0.as_slice() == b"gh_raw" {
            // Soroban -> Groundhog: gh_raw(method: u64, calldata: Bytes) -> Bytes.
            let [ScVal::U64(method), ScVal::Bytes(calldata)] = args.as_slice() else {
                return Err(fail());
            };
            return match gh_invoke(id, *method, calldata.as_slice(), &[]) {
                Ok(ret) => Ok(ScVal::Bytes(ret.try_into().map_err(|_| fail())?)),
                Err(abort) if abort.is_empty() => Err(trapped()),
                Err(_) => Err(fail()),
            };
        }
        // Soroban -> Soroban: method 0 of the callee's Groundhog contract.
        let hf = HostFunction::InvokeContract(soroban_env_host::xdr::InvokeContractArgs {
            contract_address: ScAddress::Contract(id.clone()),
            function_name: func.clone(),
            args: args.try_into().map_err(|_| fail())?,
        });
        let calldata = hf.to_xdr(Limits::none()).map_err(|_| fail())?;
        // Attach the held child token authorizing exactly this call, if any.
        let HostFunction::InvokeContract(ica) = &hf else { unreachable!() };
        let fp = auth::fingerprint(id, &ica.function_name, ica.args.as_slice());
        let tok = self.held.iter().find(|h| !h.used.get() && h.fp == fp);
        let ids: Vec<u32> = tok.iter().map(|h| h.id).collect();
        let r = gh_invoke(id, 0, &calldata, &ids);
        // On success the callee owns the token now; on failure Groundhog gave
        // it back, so it stays available.
        if let (Some(t), Ok(_)) = (tok, &r) {
            t.used.set(true);
        }
        match r {
            Ok(ret) => ScVal::from_xdr(ret, Limits::none()).map_err(|_| fail()),
            Err(abort) => match ScVal::from_xdr(abort, Limits::none()) {
                Ok(ScVal::Error(e)) => Err(e.into()),
                _ => Err(trapped()),
            },
        }
    }
}

// -------------------------------------------------------------- entrypoint --

fn invoke() {
    init_heap();
    let me = ContractId(Hash(sdk::contract_op::get_current_address().into_inner()));
    let mut calldata = sdk::internal::new_scratch();
    let mut auth_ids = sdk::contract_op::AuthTokenIdBuf::new();
    sdk::contract_op::fetch_calldata_and_auth_tokens(&mut calldata, &mut auth_ids);
    let host_fn = HostFunction::from_xdr(AsRef::<[u8]>::as_ref(&calldata), Limits::none()).unwrap();
    // This contract only runs its own Soroban contract; it is not a proxy.
    match &host_fn {
        HostFunction::InvokeContract(a) if a.contract_address == ScAddress::Contract(me.clone()) => {}
        _ => abort_with(soroban_env_host::Error::from_type_and_code(
            ScErrorType::Context, ScErrorCode::InvalidInput)),
    }

    let height = sdk::block::get_block_height();
    let ledger_info = LedgerInfo {
        protocol_version: soroban_env_host::meta::INTERFACE_VERSION.protocol,
        sequence_number: 1000 + (height.into_inner() as u32),
        timestamp: 0,
        network_id: [7; 32],
        base_reserve: 1,
        min_temp_entry_ttl: 16,
        min_persistent_entry_ttl: 4096,
        max_entry_ttl: 6_312_000,
    };
    // Auth: the token trees passed with this invocation, plus the direct
    // Groundhog caller as invoker of the root call (Soroban's invoker-contract
    // rule; `get_caller` comes from the runtime, so it can't be forged).
    let (mut entries, held) = match auth::consume_roots(&me, &auth_ids) {
        Ok(x) => x,
        Err(e) => abort_with(e),
    };
    let HostFunction::InvokeContract(root) = &host_fn else { unreachable!() };
    entries.push((
        ScAddress::Contract(ContractId(Hash(sdk::contract_op::get_caller().into_inner()))),
        soroban_env_host::xdr::SorobanAuthorizedInvocation {
            function: soroban_env_host::xdr::SorobanAuthorizedFunction::ContractFn(root.clone()),
            sub_invocations: Default::default(),
        },
    ));
    // The source account is not an auth principal here (no entry uses it).
    let source = AccountId(PublicKey::PublicKeyTypeEd25519(Uint256([0; 32])));
    let mut diag = Vec::new();
    let router = Rc::new(GroundhogRouter { me: me.clone(), held });
    let res = invoke_host_function_in_recording_mode_with_setup(
        &Budget::default(),
        false,
        &host_fn,
        &source,
        RecordingInvocationAuthMode::Enforcing(Vec::new()),
        ledger_info,
        Rc::new(HogSnapshot { me, memo: Default::default() }),
        [0; 32],
        &mut diag,
        &|host| {
            native::register(host)?;
            host.set_preverified_authorization_entries(entries.clone())?;
            host.set_external_contract_handler(router.clone())
        },
    )
    .unwrap();

    let val: ScVal = match res.invoke_result {
        Ok(v) => v,
        Err(e) => abort_with(e.error),
    };
    for ch in res.ledger_changes.iter().filter(|c| !c.read_only && !is_native_code_key(&c.encoded_key)) {
        let ttl = ch.ttl_change.as_ref().map_or(NO_TTL, |t| t.new_live_until_ledger);
        match &ch.encoded_new_value {
            Some(v) => write_entry(&ch.encoded_key, v, ttl),
            None => delete_entry(&ch.encoded_key),
        }
    }
    let out: Box<[u8]> = val.to_xdr(Limits::none()).unwrap().into();
    sdk::sys::raw_Return(&out, false, &[]);
}

/// Revert this Groundhog frame, handing the caller the Soroban error.
fn abort_with(e: soroban_env_host::Error) -> ! {
    let sc: ScError = e.try_into().unwrap_or(ScError::Context(ScErrorCode::InternalError));
    let payload = ScVal::Error(sc).to_xdr(Limits::none()).unwrap();
    sdk::contract_op::contract_abort(&payload)
}

mod rv32_entries {
    use sdk::__rv32::Entrypoint;
    extern "C" fn invoke_wrapper() {
        super::invoke();
    }
    #[used]
    #[unsafe(link_section = "groundhog_rv32_entrypoints")]
    static INVOKE: Entrypoint = Entrypoint { method_id: 0, wrapper: invoke_wrapper };

    /// Method 2: set the trusted Soroban entrypoint (once). Calldata: its
    /// 32-byte address.
    extern "C" fn set_entrypoint_wrapper() {
        super::init_heap();
        let mut calldata = sdk::internal::new_scratch();
        let mut auth_ids = sdk::contract_op::AuthTokenIdBuf::new();
        sdk::contract_op::fetch_calldata_and_auth_tokens(&mut calldata, &mut auth_ids);
        super::auth::set_trusted_entrypoint(AsRef::<[u8]>::as_ref(&calldata));
        sdk::sys::raw_Return(&[], false, &[]);
    }
    #[used]
    #[unsafe(link_section = "groundhog_rv32_entrypoints")]
    static SET_ENTRYPOINT: Entrypoint = Entrypoint { method_id: 2, wrapper: set_entrypoint_wrapper };
}
