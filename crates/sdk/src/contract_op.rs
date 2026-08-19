//! System functions
//!
//! The functions are used by the [`contract`](macro@crate::contract) macro,
//! and are also available for lower-level code.

#[cfg(not(feature = "alloc"))]
use crate::internal::MUBuf;
use crate::sys::{self, SysResult, SyscallNo};
use crate::types::{ContractAddress, RuntimeMemoryLength, SHA256Hash};
use crate::xdr::*;
use crate::{FaultString, HandlingError, syscall_throw};
use core::mem::MaybeUninit;

pub fn contract_abort(buf: &[u8]) -> ! {
    let revert = true;

    sys::raw_Return(buf, revert, &[])
}

/// Terminate the current call frame immediately, reverting it.
///
/// The caller observes a deterministic error with no return buffer, the
/// same outcome as an illegal instruction trap. To abort with a serialized
/// exception payload instead, use [`contract_abort`].
pub fn contract_revert() -> ! {
    sys::raw_Exit()
}

/// Print a debugging breadcrumb before an abort/re-throw: the verb (e.g.
/// "Throwing"/"Re-throwing"), the exception's `Debug` form, the caller's
/// location (labeled `location_label`, since call sites vary — plain
/// "Location" vs. "Interface location"), and a backtrace.
///
/// `#[track_caller]` here reports whichever site called into *this*
/// function; every call site above it in the chain also carries
/// `#[cfg_attr(feature = "debugging", track_caller)]` so that location is
/// the original user call, not an intermediate SDK frame.
#[cfg(all(feature = "debugging", feature = "std"))]
#[track_caller]
pub(crate) fn abort_breadcrumb(
    verb: &str,
    location_label: &str,
    exn: &dyn core::fmt::Debug,
) {
    std::println!();
    std::println!("  {verb} exception {:?}", exn);
    std::println!("  {location_label}: {}", core::panic::Location::caller());
    // Safety: captured and displayed in one expression on an abort path
    // (every caller of this helper never returns), so every
    // with_debug_context frame is still on the stack.
    std::println!("  Backtrace:\n{}", unsafe { crate::Backtrace::capture() });
}

#[inline]
#[cfg_attr(feature = "debugging", track_caller)]
pub fn contract_abort_encoded<T>(exn: &T) -> !
where
    T: WriteXDR + ?Sized + core::fmt::Debug,
{
    #[cfg(all(feature = "debugging", feature = "std"))]
    abort_breadcrumb("Throwing", "Location", &exn);

    _contract_abort_encoded(exn)
}

// Inlining this leads to significant code size bloat.
#[inline(never)]
fn _contract_abort_encoded<T>(exn: &T) -> !
where
    T: WriteXDR + ?Sized + core::fmt::Debug,
{
    crate::call::with_scratch(|buf| {
        crate::internal::encode_scratch(buf, exn)
            .unwrap_or_else(|e| exception_encoding_error(e));
        contract_abort(buf.as_ref());
    })
}

// A user exception failed to encode in `_contract_abort_encoded`:
// report it as a typed library exception instead of trapping. Reachable
// only from the generic `throw<E>` path — library exceptions go through
// the hand-rolled `abort_encode` encoder, which is infallible (fixed
// buffer sized to the worst-case variant) and does not touch the shared
// scratch, so no second-failure fallback is needed here anymore.
#[cold]
#[inline(never)]
fn exception_encoding_error(err: WriteError) -> ! {
    use crate::exn::LibraryExn;

    crate::exn::abort_encode::library_abort(&LibraryExn::from(
        HandlingError::EncodingException(FaultString::via_display(&err)),
    ))
}

pub fn get_caller() -> ContractAddress {
    let mut out_addr: MaybeUninit<ContractAddress> =
        MaybeUninit::<ContractAddress>::uninit();

    sys::uninit_GetMsgSender(&mut out_addr)
        .unwrap_or_else(|e| syscall_throw(SyscallNo::kGetMsgSender, e));

    unsafe { out_addr.assume_init() }
}

/// The hash of the transaction that initiated the current execution.
///
/// This is `groundhog-xdr`'s `TransactionHash`: sha256 of the XDR-encoded
/// `Transaction` — the same object the signatures are over, and the
/// identifier the transaction carries network-wide. It deliberately covers
/// neither the signatures wrapped around it nor the nondeterministic data a
/// proposer attaches, so a submitter can compute it at signing time and use
/// it to find its own transaction.
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

/// The calldata buffer — the SDK's single
/// [`ScratchBuf`](crate::internal::ScratchBuf). With the `alloc` feature this
/// is a heap `Vec` sized exactly to the calldata (any length); without it, a
/// fixed [`SCRATCH_MAX`](crate::internal::SCRATCH_MAX)-byte buffer.
///
/// Distinct from the write scratch: the decoded args borrow from this buffer
/// (`ReadXDR<'b>`) for the whole frame, so it stays pinned and cannot be
/// reused for outbound encoding.
pub type Calldata = crate::internal::ScratchBuf;

/// The auth-token-id list supplied with the invocation.
///
/// With the `alloc` feature this is sized exactly to the token count; the
/// fixed-capacity fallback supports at most 32 tokens.
#[cfg(feature = "alloc")]
pub type AuthTokenIdBuf = alloc::vec::Vec<u32>;
#[cfg(not(feature = "alloc"))]
pub type AuthTokenIdBuf = heapless::Vec<u32, 32>;

/// Fetch the current frame's calldata and auth-token ids into
/// caller-provided buffers (cleared first).
///
/// This is the fallible core of [`fetch_calldata_and_auth_tokens`]: the
/// buffers are filled in place, never moved by value. Buffers are
/// heap-allocated with their exact sizes, queried up front via
/// `GetCalldataLen`, so calldata of any length is supported.
#[cfg(feature = "alloc")]
fn fetch_calldata_and_auth_tokens_into(
    calldata: &mut Calldata,
    auth_ids: &mut AuthTokenIdBuf,
) -> SysResult<()> {
    let mut len: RuntimeMemoryLength = 0;
    let mut auth_token_count: u32 = 0;
    sys::raw_GetCalldataLen(&mut len, &mut auth_token_count)?;

    let len = len as usize;
    let auth_len = auth_token_count as usize * 4;

    calldata.clear();
    calldata.reserve(len);
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

    // Convert bytes to u32s using native byte order (rv32 is little-endian)
    auth_ids.clear();
    auth_ids.reserve(auth_token_count as usize);
    for chunk in auth_token_buf.as_chunks::<4>().0 {
        auth_ids.push(u32::from_ne_bytes(*chunk));
    }

    Ok(())
}

/// Fetch the current frame's calldata and auth-token ids into
/// caller-provided buffers (cleared first).
///
/// This is the fallible core of [`fetch_calldata_and_auth_tokens`]: the
/// buffers are filled in place, never moved by value. Without `alloc`,
/// calldata lands in the fixed
/// [`SCRATCH_MAX`](crate::internal::SCRATCH_MAX)-byte buffer and the
/// token list holds at most 32 entries; larger inputs return
/// `InvalidSyscallInput` (thrown as a `LibraryExn::Syscall` for
/// `kGetCalldata`).
#[cfg(not(feature = "alloc"))]
fn fetch_calldata_and_auth_tokens_into(
    calldata: &mut Calldata,
    auth_ids: &mut AuthTokenIdBuf,
) -> SysResult<()> {
    // 32 u32s * 4 bytes each
    const AUTH_TOKEN_IDS_MAX: usize = 32 * 4;

    calldata.clear();
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
    if written > calldata.capacity() {
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

    // Convert bytes to u32s using native byte order (rv32 is little-endian)
    auth_ids.clear();
    for chunk in auth_token_buf.as_ref().as_chunks::<4>().0 {
        let id = u32::from_ne_bytes(*chunk);
        auth_ids.push(id).expect("too many auth token IDs");
    }

    Ok(())
}

/// Fetch this frame's calldata and auth-token ids into the caller's
/// buffers (cleared first), throwing a `LibraryExn::Syscall` on fetch
/// failure. The single calldata API: generated entrypoint wrappers and
/// raw-calldata contracts both use it. The throw is guest-reachable and
/// not limited to an adversarial host: in the no-alloc guest it is
/// exactly what an ordinary transaction gets if its calldata exceeds
/// `SCRATCH_MAX` (4096) bytes or carries more than 32 auth tokens.
///
/// The buffers live in the caller's frame and are filled in place —
/// never moved by value. Generated entrypoint wrappers declare them as
/// locals and fetch exactly once, at the root of the frame; the decoded
/// args borrow the calldata buffer for the entire handler frame. In the
/// no-alloc guest that costs `SCRATCH_MAX` (4096) bytes of stack per
/// live fetch. There is no shared cell, so nested fetches (the native
/// mock's in-process frames, or a hand-written re-fetch) each get their
/// own buffers and cannot alias.
///
/// Non-generic and `#[inline(never)]`, both load-bearing for codesize:
/// every entrypoint reaches this single copy of the fetch machinery
/// (~440 bytes in the optimized artifact) by direct call — nothing
/// monomorphizes per entrypoint, and no handler's address is ever taken.
/// Without the attribute, LTO inlines the body back into each export,
/// recreating the per-export copies.
///
/// `doc(hidden)`: raw calldata access is plumbing for the generated
/// entrypoint wrappers, not a surface contracts should discover and
/// build on.
#[doc(hidden)]
#[inline(never)]
pub fn fetch_calldata_and_auth_tokens(
    calldata: &mut Calldata,
    auth_ids: &mut AuthTokenIdBuf,
) {
    fetch_calldata_and_auth_tokens_into(calldata, auth_ids).unwrap_or_else(
        |syserr| syscall_throw(SyscallNo::kGetCalldata, syserr),
    );
}
