//! Feasibility experiment: a full `soroban-env-host` running inside one
//! Groundhog RV32 contract.
//!
//! Method 0 (`invoke`): calldata is an XDR `HostFunction` (upload Wasm, create
//! contract, or invoke contract). The host runs in recording mode, so the
//! footprint is discovered on the fly: every ledger read goes through
//! `HogSnapshot` (the storage shim), which reads the entry from Groundhog
//! bytestring storage. Afterwards every ledger change is written back. The
//! return value is the XDR `ScVal` result.
//!
//! Storage layout: an XDR `LedgerEntry` can be larger than one Groundhog
//! bytestring (1 KiB by default), so each entry is split into chunks.
//!   header  @ sha256("sorhog" || LedgerKey XDR)     = len:u32 || live_until:u32
//!   chunk i @ sha256(header_key || i:u32)           = up to CHUNK bytes

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
        invoke_host_function_in_recording_mode,
    },
    storage::{EntryWithLiveUntil, SnapshotSource},
    xdr::{
        AccountId, HostFunction, LedgerEntry, LedgerKey, Limits, PublicKey, ReadXdr, ScVal,
        Uint256, WriteXdr,
    },
    HostError, LedgerInfo,
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

// ----------------------------------------------------------- storage shim --

const CHUNK: usize = 1024;
const NO_TTL: u32 = u32::MAX;

fn header_key(ledger_key_xdr: &[u8]) -> [u8; 32] {
    let mut buf = Vec::with_capacity(6 + ledger_key_xdr.len());
    buf.extend_from_slice(b"sorhog");
    buf.extend_from_slice(ledger_key_xdr);
    sdk::crypto::sha256(&buf).into_inner()
}

fn chunk_key(header: &[u8; 32], i: u32) -> StorageKey {
    let mut buf = [0u8; 36];
    buf[..32].copy_from_slice(header);
    buf[32..].copy_from_slice(&i.to_le_bytes());
    let h: [u8; 32] = sdk::crypto::sha256(&buf).into_inner();
    h.into()
}

fn read_entry(ledger_key_xdr: &[u8]) -> Option<(Vec<u8>, u32)> {
    let hk = header_key(ledger_key_xdr);
    let key: StorageKey = hk.into();
    if !sdk::bytestring_exists(&key) {
        return None;
    }
    let hdr: [u8; 8] = sdk::bytestring_get_sized(&key);
    let len = u32::from_le_bytes(hdr[..4].try_into().unwrap()) as usize;
    let live_until = u32::from_le_bytes(hdr[4..].try_into().unwrap());
    let mut out = Vec::with_capacity(len);
    let mut i = 0;
    while out.len() < len {
        out.extend_from_slice(&sdk::bytestring_get(&chunk_key(&hk, i)));
        i += 1;
    }
    Some((out, live_until))
}

fn write_entry(ledger_key_xdr: &[u8], value: &[u8], live_until: u32) {
    let hk = header_key(ledger_key_xdr);
    let old_chunks = read_header(&hk).map(|(len, _)| len.div_ceil(CHUNK)).unwrap_or(0);
    let mut hdr = [0u8; 8];
    hdr[..4].copy_from_slice(&(value.len() as u32).to_le_bytes());
    hdr[4..].copy_from_slice(&live_until.to_le_bytes());
    sdk::bytestring_set(&hk.into(), &hdr);
    let mut n = 0;
    for (i, c) in value.chunks(CHUNK).enumerate() {
        sdk::bytestring_set(&chunk_key(&hk, i as u32), c);
        n = i + 1;
    }
    for i in n..old_chunks {
        sdk::delete_shared(&chunk_key(&hk, i as u32));
    }
}

fn read_header(hk: &[u8; 32]) -> Option<(usize, u32)> {
    let key: StorageKey = (*hk).into();
    if !sdk::bytestring_exists(&key) {
        return None;
    }
    let hdr: [u8; 8] = sdk::bytestring_get_sized(&key);
    Some((
        u32::from_le_bytes(hdr[..4].try_into().unwrap()) as usize,
        u32::from_le_bytes(hdr[4..].try_into().unwrap()),
    ))
}

fn delete_entry(ledger_key_xdr: &[u8]) {
    let hk = header_key(ledger_key_xdr);
    if let Some((len, _)) = read_header(&hk) {
        for i in 0..len.div_ceil(CHUNK) {
            sdk::delete_shared(&chunk_key(&hk, i as u32));
        }
        sdk::delete_shared(&hk.into());
    }
}

/// The shim: Soroban asks for ledger entries, we answer from Groundhog storage.
struct HogSnapshot;

impl SnapshotSource for HogSnapshot {
    fn get(&self, key: &Rc<LedgerKey>) -> Result<Option<EntryWithLiveUntil>, HostError> {
        let kx = key.to_xdr(Limits::none())?;
        Ok(read_entry(&kx).map(|(bytes, live_until)| {
            let entry = LedgerEntry::from_xdr(bytes, Limits::none()).unwrap();
            let ttl = (live_until != NO_TTL).then_some(live_until);
            (Rc::new(entry), ttl)
        }))
    }
}

// -------------------------------------------------------------- entrypoint --

fn invoke() {
    init_heap();
    let mut calldata = sdk::internal::new_scratch();
    let mut auth_ids = sdk::contract_op::AuthTokenIdBuf::new();
    sdk::contract_op::fetch_calldata_and_auth_tokens(&mut calldata, &mut auth_ids);
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
    let res = invoke_host_function_in_recording_mode(
        &Budget::default(),
        false,
        &host_fn,
        &source,
        RecordingInvocationAuthMode::Recording(RecordingInvocationAuthParams::new(
            false, false,
        )),
        ledger_info,
        Rc::new(HogSnapshot),
        [0; 32],
        &mut diag,
    )
    .unwrap();

    let val: ScVal = match res.invoke_result {
        Ok(v) => v,
        Err(e) => {
            sdk::log::log(alloc::format!("soroban error: {:?}", e.error).as_bytes());
            panic!("soroban invocation failed");
        }
    };
    for ch in res.ledger_changes.iter().filter(|c| !c.read_only) {
        let ttl = ch.ttl_change.as_ref().map_or(NO_TTL, |t| t.new_live_until_ledger);
        match &ch.encoded_new_value {
            Some(v) => write_entry(&ch.encoded_key, v, ttl),
            None => delete_entry(&ch.encoded_key),
        }
    }
    let out: Box<[u8]> = val.to_xdr(Limits::none()).unwrap().into();
    sdk::sys::raw_Return(&out, false, &[]);
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
