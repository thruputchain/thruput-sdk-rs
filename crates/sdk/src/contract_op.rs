//! System functions
//!
//! The functions are used by the [`contract`](macro@crate::contract) macro,
//! and are also available for lower-level code.

use crate::call::contract_call_encode;
use crate::internal::MUBuf;
use crate::sys::{self, SysResult, SyscallNo};
use crate::types::{ContractAddress, RuntimeMemoryLength, SHA256Hash};
use crate::xdr::*;
use crate::{
    CollectAuthHandles, FaultString, HandlingError, library_throw,
    syscall_throw,
};
use core::mem::MaybeUninit;

pub(crate) const CALLDATA_MAX: usize = 500;

pub fn contract_return(buf: &[u8], auth_token_ids: &[u32]) -> ! {
    let revert = false;

    let auth_token_ids_bytes =
        crate::call::auth_handles_as_bytes(auth_token_ids);

    sys::raw_Return(buf, revert, auth_token_ids_bytes)
}

#[inline]
pub fn contract_return_buf(buf: &[u8]) -> ! {
    let auth_token_ids = &[];

    contract_return(buf, auth_token_ids)
}

pub fn contract_abort(buf: &[u8]) -> ! {
    let revert = true;

    sys::raw_Return(buf, revert, &[])
}

/// Terminate the current call frame immediately, reverting it.
///
/// The caller observes a deterministic error with no return buffer,
/// equivalent to the wasm `unreachable` instruction. To abort with a
/// serialized exception payload instead, use [`contract_abort`].
pub fn contract_revert() -> ! {
    sys::raw_Exit()
}

#[inline]
#[cfg_attr(feature = "debugging", track_caller)]
pub fn contract_abort_encoded<T>(exn: &T) -> !
where
    T: WriteXDR + ?Sized + core::fmt::Debug,
{
    #[cfg(all(feature = "debugging", feature = "std"))]
    {
        std::println!();
        std::println!("  Throwing exception {:?}", exn);
        std::println!("  Location: {}", core::panic::Location::caller());
        // Safety: captured and displayed in one expression on an abort path
        // (`_contract_abort_encoded` below never returns), so every
        // with_debug_context frame is still on the stack.
        std::println!("  Backtrace:\n{}", unsafe {
            crate::Backtrace::capture()
        });
    }

    _contract_abort_encoded(exn)
}

// Codepath for aborts in client-side codegen; coalesce with above after
// removing printlns.
#[inline]
#[cfg_attr(feature = "debugging", track_caller)]
pub fn client_contract_abort_encoded<T>(exn: &T) -> !
where
    T: WriteXDR + ?Sized + core::fmt::Debug,
{
    #[cfg(all(feature = "debugging", feature = "std"))]
    {
        std::println!();
        std::println!("  Throwing exception {:?}", exn);
        std::println!(
            "  Interface location: {}",
            core::panic::Location::caller()
        );
        // Safety: captured and displayed in one expression on an abort path
        // (`_contract_abort_encoded` below never returns), so every
        // with_debug_context frame is still on the stack.
        std::println!("  Backtrace:\n{}", unsafe {
            crate::Backtrace::capture()
        });
    }

    _contract_abort_encoded(exn)
}

// Inlining this leads to significant code size bloat.
#[inline(never)]
fn _contract_abort_encoded<T>(exn: &T) -> !
where
    T: WriteXDR + ?Sized + core::fmt::Debug,
{
    let buf = contract_call_encode(exn)
        .unwrap_or_else(|e| exception_encoding_error(e));

    contract_abort(buf.as_ref());
}

// Try once more to encode an exception before just trapping
fn exception_encoding_error(err: WriteError) -> ! {
    use crate::exn::LibraryExn;
    use crate::exn::cross_exn::ContractExn;

    let exn = ContractExn::new(LibraryExn::from(
        HandlingError::EncodingException(FaultString::via_display(&err)),
    ));
    let buf = contract_call_encode(&exn).unwrap_or_else(|_| crate::trap());

    contract_abort(buf.as_ref());
}

#[inline]
#[cfg_attr(feature = "debugging", track_caller)]
pub fn contract_return_encoded<RET>(ret: &RET) -> !
where
    RET: WriteXDR + ?Sized + CollectAuthHandles,
{
    let return_buf =
        crate::call::contract_call_encode(ret).unwrap_or_else(|e| {
            library_throw(HandlingError::EncodingException(
                FaultString::via_display(&e),
            ))
        });
    let auth_handles = crate::call::collect_auth_handle_bytes(ret)
        .unwrap_or_else(|e| {
            library_throw(HandlingError::EncodingException(
                FaultString::from_display(e),
            ));
        });

    contract_return(return_buf.as_ref(), auth_handles.as_slice());
}

pub fn get_caller() -> ContractAddress {
    let mut out_addr: MaybeUninit<ContractAddress> =
        MaybeUninit::<ContractAddress>::uninit();

    sys::uninit_GetMsgSender(&mut out_addr)
        .unwrap_or_else(|e| syscall_throw(SyscallNo::kGetMsgSender, e));

    unsafe { out_addr.assume_init() }
}

pub fn get_source_transaction_hash() -> SHA256Hash {
    let mut out_hash: MaybeUninit<SHA256Hash> =
        MaybeUninit::<SHA256Hash>::uninit();
    sys::uninit_GetSourceTransactionHash(&mut out_hash).unwrap_or_else(|e| {
        syscall_throw(SyscallNo::kGetSourceTransactionHash, e)
    });
    unsafe { out_hash.assume_init() }
}

pub fn get_current_address() -> ContractAddress {
    let mut out_addr: MaybeUninit<ContractAddress> =
        MaybeUninit::<ContractAddress>::uninit();
    sys::uninit_GetCurrentAddress(&mut out_addr)
        .unwrap_or_else(|e| syscall_throw(SyscallNo::kGetCurrentAddress, e));
    unsafe { out_addr.assume_init() }
}

pub fn try_get_calldata_len() -> SysResult<RuntimeMemoryLength> {
    let mut out: RuntimeMemoryLength = 0;
    let mut _auth_token_count: u32 = 0;
    sys::raw_GetCalldataLen(&mut out, &mut _auth_token_count)?;
    Ok(out)
}

pub fn get_calldata_len() -> RuntimeMemoryLength {
    try_get_calldata_len()
        .unwrap_or_else(|e| syscall_throw(SyscallNo::kGetCalldataLen, e))
}

pub type Calldata = MUBuf<CALLDATA_MAX>;

pub type AuthTokenIdBuf = heapless::Vec<u32, 32>;

pub fn get_calldata_with_auth_tokens() -> (Calldata, AuthTokenIdBuf) {
    try_get_calldata_with_auth_tokens().unwrap_or_else(|syserr| {
        syscall_throw(SyscallNo::kGetCalldata, syserr)
    })
}

pub fn try_get_calldata_with_auth_tokens()
-> SysResult<(Calldata, AuthTokenIdBuf)> {
    // 32 u32s * 4 bytes each
    const AUTH_TOKEN_IDS_MAX: usize = 32 * 4;

    let mut calldata: Calldata = MUBuf::new();
    let mut auth_token_buf: MUBuf<AUTH_TOKEN_IDS_MAX> = MUBuf::new();

    let mut written: MaybeUninit<RuntimeMemoryLength> = MaybeUninit::uninit();
    let mut auth_token_written: MaybeUninit<RuntimeMemoryLength> =
        MaybeUninit::uninit();

    sys::uninit_GetCalldata(
        calldata.spare_capacity_mut(),
        &mut written,
        auth_token_buf.spare_capacity_mut(),
        &mut auth_token_written,
    )?;

    // `GetCalldata` copies at most the destination span's length but reports
    // the *full* source length, which can exceed the fixed buffers here
    // (protocol calldata is unbounded). Applying an oversized length via
    // `set_len` would let `as_ref` read past the initialized region.
    let written = unsafe { written.assume_init() } as usize;
    assert!(
        written <= CALLDATA_MAX,
        "calldata length {written} exceeds the SDK calldata buffer \
         capacity {CALLDATA_MAX}"
    );
    unsafe { calldata.set_len(written) };

    let auth_written = unsafe { auth_token_written.assume_init() } as usize;
    assert!(
        auth_written <= AUTH_TOKEN_IDS_MAX,
        "auth token id list length {auth_written} exceeds the SDK auth \
         token buffer capacity {AUTH_TOKEN_IDS_MAX}"
    );
    unsafe { auth_token_buf.set_len(auth_written) };

    // Convert bytes to u32s using native byte order (wasm is little-endian)
    let mut auth_ids = AuthTokenIdBuf::new();
    for chunk in auth_token_buf.as_ref().chunks_exact(4) {
        let id = u32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        auth_ids.push(id).expect("too many auth token IDs");
    }

    Ok((calldata, auth_ids))
}
