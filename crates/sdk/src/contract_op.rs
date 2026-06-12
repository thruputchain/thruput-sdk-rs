//! System functions
//!
//! The functions are used by the [`contract`](macro@crate::contract) macro,
//! and are also available for lower-level code.

#[cfg(not(feature = "alloc"))]
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
    let buf = crate::call::encode_terminal(exn)
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
    let buf =
        crate::call::encode_terminal(&exn).unwrap_or_else(|_| crate::trap());

    contract_abort(buf.as_ref());
}

#[inline]
#[cfg_attr(feature = "debugging", track_caller)]
pub fn contract_return_encoded<RET>(ret: &RET) -> !
where
    RET: WriteXDR + ?Sized + CollectAuthHandles,
{
    let return_buf = crate::call::encode_terminal(ret).unwrap_or_else(|e| {
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

/// The calldata buffer.
///
/// With the `alloc` feature this is a heap `Vec` sized exactly to the
/// calldata (any length); without it, a fixed 500-byte (`CALLDATA_MAX`) stack
/// buffer.
#[cfg(feature = "alloc")]
pub type Calldata = alloc::vec::Vec<u8>;
#[cfg(not(feature = "alloc"))]
pub type Calldata = MUBuf<CALLDATA_MAX>;

/// The auth-token-id list supplied with the invocation.
///
/// With the `alloc` feature this is sized exactly to the token count; the
/// fixed-capacity fallback supports at most 32 tokens.
#[cfg(feature = "alloc")]
pub type AuthTokenIdBuf = alloc::vec::Vec<u32>;
#[cfg(not(feature = "alloc"))]
pub type AuthTokenIdBuf = heapless::Vec<u32, 32>;

/// Fetch the current frame's calldata and auth-token ids, throwing a
/// `LibraryExn::Syscall` on failure. See
/// [`try_get_calldata_with_auth_tokens`].
pub fn get_calldata_with_auth_tokens() -> (Calldata, AuthTokenIdBuf) {
    try_get_calldata_with_auth_tokens().unwrap_or_else(|syserr| {
        syscall_throw(SyscallNo::kGetCalldata, syserr)
    })
}

/// Fetch the current frame's calldata and auth-token ids.
///
/// Buffers are heap-allocated with their exact sizes, queried up front via
/// `GetCalldataLen`, so calldata of any length is supported.
#[cfg(feature = "alloc")]
pub fn try_get_calldata_with_auth_tokens()
-> SysResult<(Calldata, AuthTokenIdBuf)> {
    let mut len: RuntimeMemoryLength = 0;
    let mut auth_token_count: u32 = 0;
    sys::raw_GetCalldataLen(&mut len, &mut auth_token_count)?;

    let len = len as usize;
    let auth_len = auth_token_count as usize * 4;

    let mut calldata: alloc::vec::Vec<u8> =
        alloc::vec::Vec::with_capacity(len);
    let mut auth_token_buf: alloc::vec::Vec<u8> =
        alloc::vec::Vec::with_capacity(auth_len);

    let mut written: MaybeUninit<RuntimeMemoryLength> = MaybeUninit::uninit();
    let mut auth_token_written: MaybeUninit<RuntimeMemoryLength> =
        MaybeUninit::uninit();

    sys::uninit_GetCalldata(
        // `with_capacity` may over-allocate; pass exactly the spans we
        // proved initialized below.
        &mut calldata.spare_capacity_mut()[..len],
        &mut written,
        &mut auth_token_buf.spare_capacity_mut()[..auth_len],
        &mut auth_token_written,
    )?;

    // Both buffers were sized from `GetCalldataLen` in this same frame, so
    // the lengths `GetCalldata` reports must match exactly; a mismatch
    // means the host broke the syscall contract, and trusting it would let
    // `set_len` expose uninitialized bytes.
    let written = unsafe { written.assume_init() } as usize;
    let auth_written = unsafe { auth_token_written.assume_init() } as usize;
    if written != len || auth_written != auth_len {
        return Err(
            sys::SyscallErrorCode::kSyscallErrorCodeInvalidSyscallInput.into(),
        );
    }
    unsafe { calldata.set_len(written) };
    unsafe { auth_token_buf.set_len(auth_written) };

    // Convert bytes to u32s using native byte order (wasm is little-endian)
    let mut auth_ids =
        AuthTokenIdBuf::with_capacity(auth_token_count as usize);
    for chunk in auth_token_buf.chunks_exact(4) {
        auth_ids.push(u32::from_ne_bytes([
            chunk[0], chunk[1], chunk[2], chunk[3],
        ]));
    }

    Ok((calldata, auth_ids))
}

/// Fetch the current frame's calldata and auth-token ids.
///
/// Without `alloc`, calldata lands in a fixed 500-byte (`CALLDATA_MAX`) stack
/// buffer and the token list holds at most 32 entries; larger inputs return
/// `InvalidSyscallInput` (the non-try wrapper throws it as a
/// `LibraryExn::Syscall` for `kGetCalldata`).
#[cfg(not(feature = "alloc"))]
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
    // `set_len` would let `as_ref` read past the initialized region, so an
    // oversized report is surfaced as InvalidSyscallInput: the spans this
    // wrapper supplied were too small for the data.
    let written = unsafe { written.assume_init() } as usize;
    if written > CALLDATA_MAX {
        return Err(
            sys::SyscallErrorCode::kSyscallErrorCodeInvalidSyscallInput.into(),
        );
    }
    unsafe { calldata.set_len(written) };

    let auth_written = unsafe { auth_token_written.assume_init() } as usize;
    if auth_written > AUTH_TOKEN_IDS_MAX {
        return Err(
            sys::SyscallErrorCode::kSyscallErrorCodeInvalidSyscallInput.into(),
        );
    }
    unsafe { auth_token_buf.set_len(auth_written) };

    // Convert bytes to u32s using native byte order (wasm is little-endian)
    let mut auth_ids = AuthTokenIdBuf::new();
    for chunk in auth_token_buf.as_ref().chunks_exact(4) {
        let id = u32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        auth_ids.push(id).expect("too many auth token IDs");
    }

    Ok((calldata, auth_ids))
}
