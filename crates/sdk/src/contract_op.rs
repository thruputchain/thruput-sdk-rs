//! System functions
//!
//! The functions are used by the [`contract`](macro@crate::contract) macro,
//! and are also available for lower-level code.

use crate::re_throw::OrSyscallThrow;
use crate::sys::{self, SysResult, SyscallNo};
use crate::types::{ContractAddress, RuntimeMemoryLength, SHA256Hash};
use crate::xdr::*;
use crate::{FaultString, HandlingError};
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
/// `#[cfg_attr(not(target_arch = "riscv32"), track_caller)]` so that location is
/// the original user call, not an intermediate SDK frame.
#[cfg(all(not(target_arch = "riscv32"), feature = "std"))]
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
#[cfg_attr(not(target_arch = "riscv32"), track_caller)]
pub fn contract_abort_encoded<T>(exn: &T) -> !
where
    T: WriteXDR + ?Sized + core::fmt::Debug,
{
    #[cfg(all(not(target_arch = "riscv32"), feature = "std"))]
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
        .or_syscall_throw(SyscallNo::kGetMsgSender);

    unsafe { out_addr.assume_init() }
}

/// The hash of the transaction that initiated the current execution.
///
/// This is `thruput-xdr`'s `TransactionHash`: sha256 of the XDR-encoded
/// `Transaction` — the same object the signatures are over, and the
/// identifier the transaction carries network-wide. It deliberately covers
/// neither the signatures wrapped around it nor the nondeterministic data a
/// proposer attaches, so a submitter can compute it at signing time and use
/// it to find its own transaction.
pub fn get_source_transaction_hash() -> SHA256Hash {
    let mut out_hash: MaybeUninit<SHA256Hash> =
        MaybeUninit::<SHA256Hash>::uninit();
    sys::uninit_GetSourceTransactionHash(&mut out_hash)
        .or_syscall_throw(SyscallNo::kGetSourceTransactionHash);
    unsafe { out_hash.assume_init() }
}

pub fn get_current_address() -> ContractAddress {
    let mut out_addr: MaybeUninit<ContractAddress> =
        MaybeUninit::<ContractAddress>::uninit();
    sys::uninit_GetCurrentAddress(&mut out_addr)
        .or_syscall_throw(SyscallNo::kGetCurrentAddress);
    unsafe { out_addr.assume_init() }
}

pub fn try_get_calldata_len() -> SysResult<RuntimeMemoryLength> {
    let mut out: RuntimeMemoryLength = 0;
    let mut _auth_token_count: u32 = 0;
    sys::raw_GetCalldataLen(&mut out, &mut _auth_token_count)?;
    Ok(out)
}

pub fn get_calldata_len() -> RuntimeMemoryLength {
    try_get_calldata_len().or_syscall_throw(SyscallNo::kGetCalldataLen)
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

/// Fetch the current frame's calldata into the caller-provided buffer
/// (cleared first), returning the number N of auth tokens supplied with the
/// invocation. They are the frame's auth token ids 0..N-1.
///
/// This is the fallible core of [`fetch_calldata_and_auth_tokens`]: the
/// buffer is filled in place, never moved by value. It is heap-allocated
/// with its exact size, queried up front via `GetCalldataLen`, so calldata
/// of any length is supported.
#[cfg(feature = "alloc")]
fn fetch_calldata_and_auth_tokens_into(
    calldata: &mut Calldata,
) -> SysResult<u32> {
    let mut len: RuntimeMemoryLength = 0;
    let mut _auth_token_count: u32 = 0;
    sys::raw_GetCalldataLen(&mut len, &mut _auth_token_count)?;

    let len = len as usize;

    calldata.clear();
    calldata.reserve(len);

    let mut written: MaybeUninit<RuntimeMemoryLength> = MaybeUninit::uninit();
    let mut auth_token_count: MaybeUninit<u32> = MaybeUninit::uninit();

    sys::uninit_GetCalldata(
        // `reserve` may over-allocate; pass exactly the span we prove
        // initialized below.
        &mut calldata.spare_capacity_mut()[..len],
        &mut written,
        &mut auth_token_count,
    )?;

    // The buffer was sized from `GetCalldataLen` in this same frame, so the
    // length `GetCalldata` reports must match exactly; a mismatch means the
    // host broke the syscall contract, and trusting it would let `set_len`
    // expose uninitialized bytes.
    let written = unsafe { written.assume_init() } as usize;
    if written != len {
        return Err(
            sys::SyscallErrorCode::kSyscallErrorCodeInvalidSyscallInput.into(),
        );
    }
    unsafe { calldata.set_len(written) };

    Ok(unsafe { auth_token_count.assume_init() })
}

/// Fetch the current frame's calldata into the caller-provided buffer
/// (cleared first), returning the number N of auth tokens supplied with the
/// invocation. They are the frame's auth token ids 0..N-1.
///
/// This is the fallible core of [`fetch_calldata_and_auth_tokens`]: the
/// buffer is filled in place, never moved by value. Without `alloc`,
/// calldata lands in the fixed
/// [`SCRATCH_MAX`](crate::internal::SCRATCH_MAX)-byte buffer; larger
/// calldata returns `InvalidSyscallInput` (thrown as a
/// `LibraryExn::Syscall` for `kGetCalldata`).
#[cfg(not(feature = "alloc"))]
fn fetch_calldata_and_auth_tokens_into(
    calldata: &mut Calldata,
) -> SysResult<u32> {
    calldata.clear();

    let mut written: MaybeUninit<RuntimeMemoryLength> = MaybeUninit::uninit();
    let mut auth_token_count: MaybeUninit<u32> = MaybeUninit::uninit();

    sys::uninit_GetCalldata(
        calldata.spare_capacity_mut(),
        &mut written,
        &mut auth_token_count,
    )?;

    // `GetCalldata` copies at most the destination span's length but reports
    // the *full* source length, which can exceed the fixed buffer here
    // (protocol calldata is unbounded). Applying an oversized length via
    // `set_len` would let `as_ref` read past the initialized region, so an
    // oversized report is surfaced as InvalidSyscallInput: the span this
    // wrapper supplied was too small for the data.
    let written = unsafe { written.assume_init() } as usize;
    if written > calldata.capacity() {
        return Err(
            sys::SyscallErrorCode::kSyscallErrorCodeInvalidSyscallInput.into(),
        );
    }
    unsafe { calldata.set_len(written) };

    Ok(unsafe { auth_token_count.assume_init() })
}

/// Fetch this frame's calldata into the caller's buffer (cleared first),
/// returning the number N of auth tokens supplied with the invocation —
/// the frame's auth token ids 0..N-1, in the order the caller listed them —
/// and throwing a `LibraryExn::Syscall` on fetch failure. The single
/// calldata API: generated entrypoint wrappers and raw-calldata contracts
/// both use it. The throw is guest-reachable and not limited to an
/// adversarial host: in the no-alloc guest it is exactly what an ordinary
/// transaction gets if its calldata exceeds `SCRATCH_MAX` (4096) bytes.
///
/// The buffer lives in the caller's frame and is filled in place — never
/// moved by value. Generated entrypoint wrappers declare it as a local and
/// fetch exactly once, at the root of the frame; the decoded args borrow
/// the calldata buffer for the entire handler frame. In the no-alloc guest
/// that costs `SCRATCH_MAX` (4096) bytes of stack per live fetch. There is
/// no shared cell, so nested fetches (the native mock's in-process frames,
/// or a hand-written re-fetch) each get their own buffer and cannot alias.
///
/// Non-generic and `#[inline(never)]`, both load-bearing for codesize:
/// every entrypoint reaches this single copy of the fetch machinery by
/// direct call — nothing monomorphizes per entrypoint, and no handler's
/// address is ever taken. Without the attribute, LTO inlines the body back
/// into each export, recreating the per-export copies.
///
/// `doc(hidden)`: raw calldata access is plumbing for the generated
/// entrypoint wrappers, not a surface contracts should discover and
/// build on.
#[doc(hidden)]
#[inline(never)]
pub fn fetch_calldata_and_auth_tokens(calldata: &mut Calldata) -> u32 {
    fetch_calldata_and_auth_tokens_into(calldata)
        .or_syscall_throw(SyscallNo::kGetCalldata)
}
