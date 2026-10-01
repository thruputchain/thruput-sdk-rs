//! Experiment: soroban-env-host *and* the Soroban contracts compiled natively
//! to RV32, in one Groundhog contract. No Wasm engine runs: contracts listed in
//! `NATIVE_CONTRACTS` are registered with the host as native function sets,
//! keyed by a pseudo Wasm hash, so the normal Soroban lifecycle still applies
//! (`CreateContract` with `ContractExecutable::Wasm(<pseudo hash>)`, then
//! `InvokeContract`). The host dispatches calls straight to the contract's
//! `#[contractimpl]`-generated `__<Type>__<fn>__invoke_raw` functions.
//!
//! Method 0 (`invoke`): calldata is an XDR `HostFunction`, run in recording
//! mode. Ledger reads go through `HogSnapshot` (the storage shim) into
//! Groundhog bytestring storage; ledger changes are written back after.
//! Storage layout: see the storage shim section (chunked entries, header
//! folded into chunk 0 so small entries cost a single Groundhog key).

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
        AccountId, HostFunction, LedgerEntry, LedgerKey, Limits, PublicKey, ReadXdr, ScVal,
        Uint256, WriteXdr,
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
    #[unsafe(no_mangle)]
    extern "C" fn sys_panic(ptr: *const u8, len: usize) -> ! {
        sdk::log::log(unsafe { core::slice::from_raw_parts(ptr, len) });
        trap()
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

// -------------------------------------------------------------- profiling --

/// Gas checkpoints for method 1 (`invoke_profiled`): phase marks plus gas
/// accumulated inside the storage shim and inside native contract code.
mod prof {
    use alloc::vec::Vec;
    pub static MARKS: spin::Mutex<Vec<(u8, u64)>> = spin::Mutex::new(Vec::new());
    pub static SHIM_GAS: spin::Mutex<(u64, u64)> = spin::Mutex::new((0, 0)); // (gas, calls)
    pub static CONTRACT_GAS: spin::Mutex<u64> = spin::Mutex::new(0);
    /// One record per shim read: LedgerKey discriminant, found flag, and for
    /// ContractData the durability + whether the key is the instance key.
    pub static READS: spin::Mutex<Vec<[u8; 3]>> = spin::Mutex::new(Vec::new());
    pub fn now() -> u64 {
        sdk::stats::gas_remaining()
    }
    pub fn mark(id: u8) {
        let g = now();
        MARKS.lock().push((id, g));
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
/// Soroban asks for the same few keys several times per transaction (during
/// the call, then again when it assembles the footprint), so answers are
/// memoised for the transaction. Code entries for natively compiled contracts
/// are synthesised: their "code" is in this binary, not in storage.
#[derive(Default)]
struct HogSnapshot {
    memo: core::cell::RefCell<alloc::collections::BTreeMap<Vec<u8>, Option<EntryWithLiveUntil>>>,
}

impl SnapshotSource for HogSnapshot {
    fn get(&self, key: &Rc<LedgerKey>) -> Result<Option<EntryWithLiveUntil>, HostError> {
        let g0 = prof::now();
        let kx = key.to_xdr(Limits::none())?;
        if let Some(hit) = self.memo.borrow().get(&kx) {
            prof::SHIM_GAS.lock().0 += g0 - prof::now();
            return Ok(hit.clone());
        }
        let r = match key.as_ref() {
            LedgerKey::ContractCode(cc) if native::is_native_hash(&cc.hash.0) => {
                Some((Rc::new(native::code_stub(cc.hash.clone())), Some(u32::MAX)))
            }
            _ => read_entry(&kx).map(|(bytes, live_until)| {
                let entry = LedgerEntry::from_xdr(bytes, Limits::none()).unwrap();
                let ttl = (live_until != NO_TTL).then_some(live_until);
                (Rc::new(entry), ttl)
            }),
        };
        self.memo.borrow_mut().insert(kx, r.clone());
        let tag = match key.as_ref() {
            LedgerKey::Account(_) => 0u8,
            LedgerKey::Trustline(_) => 1,
            LedgerKey::ContractData(_) => 6,
            LedgerKey::ContractCode(_) => 7,
            LedgerKey::Ttl(_) => 9,
            _ => 255,
        };
        let inst = matches!(key.as_ref(), LedgerKey::ContractData(cd)
            if matches!(cd.key, ScVal::LedgerKeyContractInstance)) as u8;
        prof::READS.lock().push([tag, r.is_some() as u8, inst]);
        let mut sg = prof::SHIM_GAS.lock();
        sg.0 += g0 - prof::now();
        sg.1 += 1;
        Ok(r)
    }
}

// ------------------------------------------------------- native contracts --

mod native {
    use super::*;
    use soroban_env_host::{ContractFunctionSet, EnvBase as _, Symbol, Val};

    /// Pseudo Wasm hash a native contract is registered under. Creating a
    /// Soroban contract with `ContractExecutable::Wasm(native_hash(label))`
    /// binds it to that native code.
    pub fn native_hash(label: &str) -> [u8; 32] {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"sorhog-native:");
        buf.extend_from_slice(label.as_bytes());
        sdk::crypto::sha256(&buf).into_inner()
    }

    pub fn is_native_hash(h: &[u8; 32]) -> bool {
        contract_labels().iter().any(|l| native_hash(l) == *h)
    }

    /// The code entry the host expects for a native contract: no Wasm bytes,
    /// keyed by its pseudo hash.
    pub fn code_stub(hash: soroban_env_host::xdr::Hash) -> LedgerEntry {
        use soroban_env_host::xdr::{
            ContractCodeEntry, ContractCodeEntryExt, LedgerEntryData, LedgerEntryExt,
        };
        LedgerEntry {
            last_modified_ledger_seq: 0,
            data: LedgerEntryData::ContractCode(ContractCodeEntry {
                ext: ContractCodeEntryExt::V0,
                hash,
                code: Default::default(),
            }),
            ext: LedgerEntryExt::V0,
        }
    }

    /// Builds a `ContractFunctionSet` dispatching (small) function symbols to
    /// a `#[contractimpl]`'s generated native entrypoints.
    macro_rules! native_contract {
        ($name:ident, $krate:ident :: $ty:ident { $($func:ident => $raw:ident ($($arg:tt),*)),* $(,)? }) => {
            pub struct $name;
            impl ContractFunctionSet for $name {
                #[allow(deprecated)]
                fn call(&self, func: &Symbol, host: &Host, args: &[Val]) -> Option<Val> {
                    let env = soroban_sdk::Env::__from_host(host.clone());
                    $(
                        if func.to_val().get_payload()
                            == Symbol::try_from_small_str(stringify!($func)).ok()?.to_val().get_payload()
                        {
                            let n = [$($arg),*].len();
                            if args.len() != n {
                                return None;
                            }
                            let g0 = crate::prof::now();
                            let r = $krate::$raw(env, $(args[$arg]),*);
                            *crate::prof::CONTRACT_GAS.lock() += g0 - crate::prof::now();
                            return Some(r);
                        }
                    )*
                    None
                }
            }
        };
    }

    native_contract!(CounterFns, soroban_counter::Counter {
        increment => __Counter__increment__invoke_raw(0),
    });

    fn contract_labels() -> [&'static str; 1] {
        ["soroban-counter"]
    }

    /// (label, function set) for every contract compiled into this binary.
    fn contracts() -> [(&'static str, Rc<dyn ContractFunctionSet>); 1] {
        [("soroban-counter", Rc::new(CounterFns))]
    }

    pub fn register_all(host: &Host) -> Result<(), HostError> {
        for (label, fns) in contracts() {
            let hash = host.bytes_new_from_slice(&native_hash(label))?;
            host.register_native_contract_as_wasm(fns, hash)?;
        }
        Ok(())
    }
}

fn is_native_code_key(key_xdr: &[u8]) -> bool {
    matches!(LedgerKey::from_xdr(key_xdr, Limits::none()),
        Ok(LedgerKey::ContractCode(cc)) if native::is_native_hash(&cc.hash.0))
}

// -------------------------------------------------------------- entrypoint --

fn invoke(profiled: bool) {
    let g_entry = prof::now();
    init_heap();
    prof::MARKS.lock().push((0, g_entry));
    let mut calldata = sdk::internal::new_scratch();
    let mut auth_ids = sdk::contract_op::AuthTokenIdBuf::new();
    sdk::contract_op::fetch_calldata_and_auth_tokens(&mut calldata, &mut auth_ids);
    prof::mark(1);
    let host_fn = HostFunction::from_xdr(AsRef::<[u8]>::as_ref(&calldata), Limits::none()).unwrap();

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
    // Everything runs as one fixed Soroban "source account"; mapping Groundhog
    // auth onto Soroban auth is out of scope for this experiment.
    let source = AccountId(PublicKey::PublicKeyTypeEd25519(Uint256([1; 32])));
    let mut diag = Vec::new();
    prof::mark(2);
    let budget = Budget::default();
    prof::mark(3);
    let res = invoke_host_function_in_recording_mode_with_setup(
        &budget,
        false,
        &host_fn,
        &source,
        RecordingInvocationAuthMode::Recording(RecordingInvocationAuthParams::new(
            false, false,
        )),
        ledger_info,
        Rc::new(HogSnapshot::default()),
        [0; 32],
        &mut diag,
        &|host| {
            prof::mark(4);
            native::register_all(host)?;
            prof::mark(5);
            Ok(())
        },
    )
    .unwrap();
    prof::mark(6);

    let val: ScVal = match res.invoke_result {
        Ok(v) => v,
        Err(e) => {
            sdk::log::log(alloc::format!("soroban error: {:?}", e.error).as_bytes());
            panic!("soroban invocation failed");
        }
    };
    for ch in res.ledger_changes.iter().filter(|c| !c.read_only && !is_native_code_key(&c.encoded_key)) {
        let ttl = ch.ttl_change.as_ref().map_or(NO_TTL, |t| t.new_live_until_ledger);
        match &ch.encoded_new_value {
            Some(v) => write_entry(&ch.encoded_key, v, ttl),
            None => delete_entry(&ch.encoded_key),
        }
    }
    prof::mark(7);
    let mut out = val.to_xdr(Limits::none()).unwrap();
    if profiled {
        // result XDR || u8 count || (u8 mark, u64 gas_remaining)* || shim gas,
        // shim calls, contract gas (u64 each). All little-endian.
        prof::mark(8);
        let marks = prof::MARKS.lock();
        out.push(marks.len() as u8);
        for (id, g) in marks.iter() {
            out.push(*id);
            out.extend_from_slice(&g.to_le_bytes());
        }
        let sg = prof::SHIM_GAS.lock();
        out.extend_from_slice(&sg.0.to_le_bytes());
        out.extend_from_slice(&sg.1.to_le_bytes());
        out.extend_from_slice(&prof::CONTRACT_GAS.lock().to_le_bytes());
        let reads = prof::READS.lock();
        for r in reads.iter() {
            out.extend_from_slice(r);
        }
        out.push(reads.len() as u8);
    }
    let out: Box<[u8]> = out.into();
    sdk::sys::raw_Return(&out, false, &[]);
}

mod rv32_entries {
    use sdk::__rv32::Entrypoint;
    extern "C" fn invoke_wrapper() {
        super::invoke(false);
    }
    extern "C" fn invoke_profiled_wrapper() {
        super::invoke(true);
    }
    #[used]
    #[unsafe(link_section = "groundhog_rv32_entrypoints")]
    static INVOKE: Entrypoint = Entrypoint { method_id: 0, wrapper: invoke_wrapper };
    #[used]
    #[unsafe(link_section = "groundhog_rv32_entrypoints")]
    static INVOKE_PROFILED: Entrypoint =
        Entrypoint { method_id: 1, wrapper: invoke_profiled_wrapper };
}
