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
        RecordingInvocationAuthMode, RecordingInvocationAuthParams,
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

// ----------------------------------------------------------- call routing --

/// Routes Soroban calls that leave this contract to Groundhog invocations.
struct GroundhogRouter {
    me: ContractId,
}

/// What a caller sees when the callee's frame trapped (a native panic): the
/// same error a trapped Wasm callee produces.
fn trapped() -> soroban_env_host::Error {
    soroban_env_host::Error::from_type_and_code(ScErrorType::WasmVm, ScErrorCode::InvalidAction)
}

fn gh_invoke(target: &ContractId, method: u64, calldata: &[u8]) -> Result<Vec<u8>, Vec<u8>> {
    use sdk::call::{contract_call, CallOutcome};
    let mut ret = Vec::new();
    let mut ret_auth = Vec::new();
    let outcome = contract_call(
        &sdk::types::ContractAddress::from(target.0 .0),
        sdk::contract::ContractEntrypointID::from_u64(method),
        u64::MAX,
        calldata,
        &Vec::new(),
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
            return match gh_invoke(id, *method, calldata.as_slice()) {
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
        match gh_invoke(id, 0, &calldata) {
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
    // Auth is still recording-mode (everything authorised); mapping Groundhog
    // callers onto Soroban auth is the next problem, not solved here.
    let source = AccountId(PublicKey::PublicKeyTypeEd25519(Uint256([1; 32])));
    let mut diag = Vec::new();
    let router = Rc::new(GroundhogRouter { me: me.clone() });
    let res = invoke_host_function_in_recording_mode_with_setup(
        &Budget::default(),
        false,
        &host_fn,
        &source,
        RecordingInvocationAuthMode::Recording(RecordingInvocationAuthParams::new(false, false)),
        ledger_info,
        Rc::new(HogSnapshot { me, memo: Default::default() }),
        [0; 32],
        &mut diag,
        &|host| {
            native::register(host)?;
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
}
