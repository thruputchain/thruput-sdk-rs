//! Making contract calls

use super::contract_result::*;
use super::encode::*;
use super::raw_call_error::*;
use crate::contract::ContractEntrypointID;
use crate::exn::{FaultString, RawExceptionRef};
use crate::types::transaction::{
    InvocationFrameStatusCode, kMaxReturnBufferLen,
};
use crate::types::{ContractAddress, RuntimeMemoryLength};
use crate::xdr::*;
use crate::{AuthHandleBuf, sys};

/// Typed cross-contract call with caller-supplied exception dispatch.
///
/// `on_exn` runs while the abort bytes are still borrowed from the call
/// scratch: [`RawExceptionRef::payload`] is a plain slice, valid exactly for
/// the closure. Decode it into owned typed errors (zero-copy),
/// [`RawExceptionRef::re_throw`] it (zero-copy), or
/// [`RawExceptionRef::to_owned`] it (one copy) — whatever `X` the closure
/// returns is what escapes.
///
/// Any SDK op performed inside `on_exn` (a nested call, hashing, emitting an
/// event) hits the cold fresh-buffer fallback, since the scratch is already
/// borrowed here (no-alloc: a 4 KiB stack buffer) — keep the closure small:
/// decode, `re_throw`, or `to_owned` only.
#[cfg_attr(feature = "debugging", track_caller)]
pub fn try_call_raw_with<ARGS, RET, X>(
    address: &ContractAddress,
    entrypoint: ContractEntrypointID,
    gas_limit: u64,
    args: &ARGS,
    flags: crate::invoke_config::InvokeFlagSet,
    on_exn: impl for<'s> FnOnce(RawExceptionRef<'s>) -> X,
) -> Result<RET, RawCallError<X>>
where
    ARGS: WriteXDR + crate::CollectAuthHandles,
    RET: ReadXDROwned + crate::ReceiveAuthHandles,
{
    with_call_scratch(|buf, auth_buf| {
        encode_args_into(buf, args).map_err(widen_uninhabited_exn)?;
        args.collect_auth_handles(auth_buf)
            .map_err(RawCallError::EncodingArgs)?;

        let is_ok =
            call_core(address, entrypoint, gas_limit, flags, buf, auth_buf)
                .map_err(widen_uninhabited_exn)?;

        if is_ok {
            contract_call_decode(buf.as_slice(), auth_buf.as_slice())
                .map_err(widen_uninhabited_exn)
        } else {
            match RawExceptionRef::parse(buf.as_slice()) {
                Ok(exn) => Err(RawCallError::Exception(on_exn(exn))),
                Err(e) => Err(RawCallError::UnparseableAbort(
                    FaultString::from_display(e),
                )),
            }
        }
    })
}

/// CXS-encode `value` into the (cleared) scratch data buffer.
///
/// Under `alloc` the buffer grows organically to the encoding's size (no
/// fixed cap — protocol calldata is unbounded) and the grown capacity is
/// retained by the scratch for subsequent calls. Without `alloc` the
/// fixed-capacity buffer rejects encodings larger than `SCRATCH_MAX`
/// (4096) bytes.
fn encode_args_into<T: WriteXDR + ?Sized>(
    buf: &mut Buf,
    value: &T,
) -> Result<(), CallFault> {
    // Cleared here rather than left to `encode_scratch`: its no-alloc branch
    // encodes into the spare capacity *past* `buf`'s current length.
    buf.clear();
    crate::internal::encode_scratch(buf, value)
        .map_err(|e| RawCallError::EncodingArgs(FaultString::from_display(e)))
}

/// The whole call orchestration, with no type parameters: invoke the child
/// frame and land whatever it produced (return value or abort payload) in
/// `buf`, its returned auth-token ids in `auth_buf`. Reports whether the
/// child returned (`true`) or aborted (`false`).
///
/// `buf` is in-out and holds the encoded calldata on entry: the `Invoke`
/// syscall has already copied the args out by the time the child's payload
/// is fetched, so one scratch buffer serves both directions (which is why
/// this can't take the args as a separate `&[u8]`, as [`contract_call`]
/// does — that would alias the same buffer).
///
/// [`try_call_raw_with`] is generic over `(ARGS, RET, X)` and every generated
/// client instantiates it afresh, so anything left in its body is re-emitted
/// per call form — yet only arg encoding, return decoding, and the caller's
/// `on_exn` genuinely depend on those parameters. `inline(never)` keeps this
/// to one copy.
///
/// On its own that is a *net loss* (measured: +116 bytes across the contract
/// suite) — hoisting the orchestration out of the caller costs more than the
/// sharing saves. It pays (-1452) only together with the single-call-site
/// scratch checkout, which lets what remains of the closure inline into the
/// contract method. Don't judge this function in isolation.
#[inline(never)]
fn call_core(
    address: &ContractAddress,
    entrypoint: ContractEntrypointID,
    gas_limit: u64,
    flags: crate::invoke_config::InvokeFlagSet,
    buf: &mut Buf,
    auth_buf: &mut AuthHandleBuf,
) -> Result<bool, CallFault> {
    let (is_ok, outputs) = invoke_frame(
        address,
        entrypoint,
        gas_limit,
        buf.as_slice(),
        auth_buf,
        flags,
    )?;

    buf.clear();

    // No `reserve` here, unlike `contract_call`: `reset_call_scratch` already
    // sized the scratch to `SCRATCH_MAX`, which `invoke_frame` has checked the
    // payload against (`kMaxReturnBufferLen`). `contract_call`'s buffer is
    // caller-supplied and has no such guarantee.
    fetch_frame_payload(is_ok, &outputs, buf, auth_buf)?;

    Ok(is_ok)
}

/// Pull the child frame's payload — returned value or abort bytes — out of
/// the VM into `buf`, and its returned auth-token ids into `auth_buf`, sized
/// per the lengths the `Invoke` syscall reported.
///
/// The one place that reads `CopyableInvokeOutputs`, shared by [`call_core`]
/// and [`contract_call`] (which otherwise cannot share a body: one owns its
/// scratch, the other writes into a caller's buffer). A new output field or
/// validation lands here once, for both.
#[inline]
fn fetch_frame_payload(
    is_ok: bool,
    outputs: &sys::CopyableInvokeOutputs,
    buf: &mut Buf,
    auth_buf: &mut AuthHandleBuf,
) -> Result<(), CallFault> {
    resize_auth_handles(auth_buf, outputs)?;
    fill_return_buf(
        outputs.return_buffer_len,
        outputs.returned_auth_token_len,
        buf,
        auth_buf,
    )
    .map_err(|fault| unparseable(is_ok, fault))
}

/// How a child frame that ran to completion ended. Produced by
/// [`contract_call`]; the payload for both variants lives in the
/// caller-provided return buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallOutcome {
    /// The child returned: `return_buf` holds the returned bytes and the
    /// auth-handle buffer the returned token ids.
    Return,
    /// The child aborted: `return_buf` holds the abort payload.
    Abort,
}

/// Untyped cross-contract call: pass pre-encoded calldata, receive the raw
/// return (or abort) bytes in `return_buf` and the returned auth-token ids
/// in `returned_auth_token_ids`. Both buffers are cleared before the call is
/// attempted, so they are left empty (not stale from a previous call) even
/// if this returns `Err`.
///
/// Prefer [`try_call_raw_with`] (or a generated client) for ordinary typed
/// calls. This entry point exists for forwarders and proxies — e.g. wallet
/// contracts that relay opaque calldata to a target and pass the raw
/// return/abort buffer back verbatim — which cannot name the argument or
/// return types. The payload lands in the caller's buffers rather than
/// being returned by value: the no-alloc buffer is `SCRATCH_MAX` bytes,
/// and a by-value return would copy it once per call layer.
///
/// The error type is `RawCallError<Infallible>`: aborts surface as
/// [`CallOutcome::Abort`] with the payload in `return_buf`, so the
/// `Exception` variant is uninhabited here — which also keeps the `Result`
/// slot small (a `RawException` payload would cost the no-alloc caller
/// a `SCRATCH_MAX`-sized stack slot for a variant this call never builds).
// The two trailing out-params are the point of the API (no by-value buffer
// returns); folding the six real inputs into a struct would be churn, not
// clarity.
#[allow(clippy::too_many_arguments)]
pub fn contract_call<A>(
    address: &ContractAddress,
    entrypoint: ContractEntrypointID,
    gas_limit: u64,
    args: &A,
    auth_token_ids: &crate::collect_auth_handles::AuthHandleBuf,
    flags: crate::invoke_config::InvokeFlagSet,
    return_buf: &mut ReturnBuf,
    returned_auth_token_ids: &mut AuthHandleBuf,
) -> Result<CallOutcome, CallFault>
where
    A: AsRef<[u8]> + ?Sized,
{
    return_buf.clear();
    returned_auth_token_ids.clear();

    let (is_ok, outputs) = invoke_frame(
        address,
        entrypoint,
        gas_limit,
        args.as_ref(),
        auth_token_ids,
        flags,
    )?;

    #[cfg(feature = "alloc")]
    return_buf.reserve(outputs.return_buffer_len as usize);

    // `fetch_frame_payload` grows `returned_auth_token_ids` to its final
    // length (zero-filled placeholders for `fill_return_buf`'s syscall to
    // overwrite) before any of the actual parsing/validation can fail — and
    // `fill_return_buf` itself sets `return_buf`'s length before its own
    // trailing auth-handle-length check can fail. Every early return through
    // there would otherwise leave one or both out-buffers non-empty despite
    // the documented "cleared on Err" contract, so any failure clears both
    // before propagating.
    if let Err(e) = fetch_frame_payload(
        is_ok,
        &outputs,
        return_buf,
        returned_auth_token_ids,
    ) {
        return_buf.clear();
        returned_auth_token_ids.clear();
        return Err(e);
    }

    Ok(if is_ok {
        CallOutcome::Return
    } else {
        CallOutcome::Abort
    })
}

/// Widen a `RawCallError` whose `Exception` variant is uninhabited
/// ([`core::convert::Infallible`]) to any `X`: the `Exception` arm can
/// never actually be constructed, so this just relabels the type.
pub(crate) fn widen_uninhabited_exn<X>(e: CallFault) -> RawCallError<X> {
    match e {
        RawCallError::Invalid => RawCallError::Invalid,
        RawCallError::Exception(x) => match x {},
        RawCallError::Fault(f) => RawCallError::Fault(f),
        RawCallError::UnparseableReturn(f) => {
            RawCallError::UnparseableReturn(f)
        }
        RawCallError::UnparseableAbort(f) => RawCallError::UnparseableAbort(f),
        RawCallError::UnparseableAuthHandles(f) => {
            RawCallError::UnparseableAuthHandles(f)
        }
        RawCallError::EncodingArgs(f) => RawCallError::EncodingArgs(f),
    }
}

/// Map a returned-data parse fault to the error variant matching the
/// frame's outcome (success vs abort).
fn unparseable(is_ok: bool, fault: FaultString) -> CallFault {
    if is_ok {
        RawCallError::UnparseableReturn(fault)
    } else {
        RawCallError::UnparseableAbort(fault)
    }
}

/// Perform the `Invoke` syscall and interpret the frame status, returning
/// whether the child returned (vs aborted) alongside the syscall outputs
/// (which carry the sizes of the data waiting in the VM's return buffer).
fn invoke_frame(
    address: &ContractAddress,
    entrypoint: ContractEntrypointID,
    gas_limit: u64,
    args: &[u8],
    auth_token_ids: &crate::collect_auth_handles::AuthHandleBuf,
    flags: crate::invoke_config::InvokeFlagSet,
) -> Result<(bool, sys::CopyableInvokeOutputs), CallFault> {
    // Perform the cross-contract call
    let entrypoint = u64::from_be_bytes(entrypoint.into_inner());
    let auth_bytes = auth_handles_as_bytes(auth_token_ids.as_slice());
    let inputs = sys::CopyableInvokeInputs {
        entrypoint_id: entrypoint,
        gas_limit,
        flags,
    };
    let mut outputs = sys::CopyableInvokeOutputs {
        return_buffer_len: 0,
        returned_auth_token_len: 0,
        // Placeholder; overwritten by the syscall whenever a child frame runs
        // (i.e. when `raw_Invoke` returns `Ok`). Never read on the error path.
        frame_status: InvocationFrameStatusCode::Fault,
    };
    let result = {
        // The callee has its own memory on a real guest, so the caller's
        // live guards must not leak into the callee's terminal-return
        // check. Restored when the scope ends, including on unwind.
        let _reset = crate::storage::live_guards::FrameGuardReset::new();
        sys::raw_Invoke(address, args, auth_bytes, &inputs, &mut outputs)
    };

    let is_ok = match result {
        // A child frame ran; its outcome is reported in `frame_status`.
        Ok(()) => {
            let status = outputs.frame_status;
            if status == InvocationFrameStatusCode::Return {
                true
            } else if status == InvocationFrameStatusCode::Abort {
                false
            } else {
                // Fault (trap / out of gas) or an unexpected status code.
                return Err(RawCallError::Fault(FaultString::from_display(
                    format_args!("child frame faulted: {status:?}"),
                )));
            }
        }
        // The call was rejected before any child frame ran.
        Err(syserr) if syserr.is_invalid_input() => {
            return Err(RawCallError::Invalid);
        }
        Err(syserr) => {
            return Err(RawCallError::Fault(FaultString::from_display(
                format_args!("{syserr:?}"),
            )));
        }
    };

    // The protocol caps returns at kMaxReturnBufferLen; a larger reported
    // length means the host broke the syscall contract.
    if outputs.return_buffer_len > kMaxReturnBufferLen {
        return Err(unparseable(
            is_ok,
            FaultString::from_display(format_args!(
                "reported return length {} exceeds the protocol cap {}",
                outputs.return_buffer_len, kMaxReturnBufferLen
            )),
        ));
    }

    Ok((is_ok, outputs))
}

/// Size the auth-handle buffer to hold the token-id list the syscall
/// outputs report.
///
/// The runtime guarantees the reported byte length is a multiple of four
/// (the list is `u32` token ids), so the division below never discards
/// trailing bytes.
fn resize_auth_handles(
    buf: &mut AuthHandleBuf,
    outputs: &sys::CopyableInvokeOutputs,
) -> Result<(), CallFault> {
    debug_assert!(
        (outputs.returned_auth_token_len as usize)
            .is_multiple_of(core::mem::size_of::<u32>()),
        "runtime reported an auth-token byte length not divisible by four"
    );
    let count =
        outputs.returned_auth_token_len as usize / core::mem::size_of::<u32>();
    #[cfg(feature = "alloc")]
    {
        buf.resize(count, 0);
        Ok(())
    }
    #[cfg(not(feature = "alloc"))]
    {
        buf.resize(count, 0).map_err(|_| {
            RawCallError::UnparseableAuthHandles(FaultString::from_display(
                "auth handle buffer capacity exceeded",
            ))
        })
    }
}

#[cfg(feature = "alloc")]
use crate::cell::{ContractLocalRef, ContractRefCell};

/// The per-instance call scratch: the shared write buffer and the
/// auth-handle list.
struct CallScratch {
    buf: Buf,
    auth: AuthHandleBuf,
}

/// Run `f` with a call scratch (encoded-args / returned-data, and the
/// auth-token-id list).
///
/// The scratch is the SDK's write buffer: cross-contract calls, event
/// emission, storage writes, and terminal returns all encode through it
/// (calldata is the one separate buffer, since decoded args borrow from it for
/// the whole frame). Scratch uses nest — an exception thrown mid-encode
/// re-enters to encode the abort, and the native mock runs child frames
/// in-process — so each use needs its own buffer, not a shared one.
///
/// No-alloc: that buffer is just a local. `Buf` is a `MaybeUninit` array, so
/// constructing one costs a length store and no `memset`; there is nothing to
/// cache and nothing a static would save. Nesting is then correct by
/// construction — no cell, no borrow flag, no re-entrancy fallback.
#[cfg(not(feature = "alloc"))]
fn with_call_scratch<R>(
    f: impl FnOnce(&mut Buf, &mut AuthHandleBuf) -> R,
) -> R {
    let mut scratch = fresh_call_scratch();
    f(&mut scratch.buf, &mut scratch.auth)
}

/// Run `f` with a call scratch. See the no-alloc twin for what the scratch is.
///
/// Under `alloc` a local would mean a fresh `Vec` allocation per use, and the
/// guest bump allocator never frees — so buffers are pooled instead and their
/// capacity reused. A use *takes* its buffer out of the pool for the duration
/// (the pool's `RefCell` is never held while `f` runs), so a nested use simply
/// takes the next one, and the buffers come back on the way out. Moving a
/// `CallScratch` here moves two `Vec` headers, not their contents.
///
/// A `f` that never returns (a terminal return, an abort) drops its buffer
/// rather than returning it to the pool. That is fine: the frame is over.
#[cfg(feature = "alloc")]
fn with_call_scratch<R>(
    f: impl FnOnce(&mut Buf, &mut AuthHandleBuf) -> R,
) -> R {
    let mut scratch = SCRATCH_POOL
        .with_ref(|pool| pool.borrow_mut().pop())
        .unwrap_or_else(fresh_call_scratch);

    reset_call_scratch(&mut scratch);
    let out = f(&mut scratch.buf, &mut scratch.auth);

    SCRATCH_POOL.with_ref(|pool| pool.borrow_mut().push(scratch));
    out
}

#[cfg(feature = "alloc")]
crate::contract_local! {
    static SCRATCH_POOL: ContractRefCell<alloc::vec::Vec<CallScratch>> =
        const { ContractRefCell::new(alloc::vec::Vec::new()) };
}

/// Ready a pooled scratch for a fresh use: cleared, and grown to `SCRATCH_MAX`
/// once so in-bounds payloads never reallocate.
#[cfg(feature = "alloc")]
#[inline]
fn reset_call_scratch(scratch: &mut CallScratch) {
    scratch.buf.clear();
    scratch.auth.clear();
    if scratch.buf.capacity() < crate::internal::SCRATCH_MAX {
        scratch.buf.reserve(crate::internal::SCRATCH_MAX);
    }
}

fn fresh_call_scratch() -> CallScratch {
    CallScratch {
        buf: crate::internal::new_scratch(),
        auth: AuthHandleBuf::new(),
    }
}

/// Run `f` with just the data scratch buffer. For encode-then-syscall sites
/// (events, storage values, terminal returns) that don't touch the auth-handle
/// list.
pub(crate) fn with_scratch<R>(f: impl FnOnce(&mut Buf) -> R) -> R {
    with_call_scratch(|buf, _auth| f(buf))
}

#[cfg(all(test, not(target_arch = "riscv32"), feature = "std"))]
mod scratch_tests {
    use super::*;

    /// A nested scratch use — a throw while the outer scratch is mid-encode,
    /// or the native mock's in-process child frames — must get its *own*
    /// buffer, or the inner encode would scribble over the outer one. Under
    /// `alloc` that means the pool hands out a second buffer rather than
    /// re-lending the first.
    #[test]
    fn nested_scratch_uses_get_distinct_buffers() {
        with_scratch(|outer| {
            outer.clear();
            outer.extend_from_slice(&[0xAA; 8]);
            let outer_ptr = outer.as_slice().as_ptr();

            let inner_ptr = with_scratch(|inner| {
                inner.clear();
                inner.extend_from_slice(&[0xBB; 8]);
                inner.as_slice().as_ptr()
            });

            assert_ne!(
                outer_ptr, inner_ptr,
                "nested scratch use aliased the outer buffer"
            );
            // The outer buffer survived the nested use untouched.
            assert_eq!(outer.as_slice(), &[0xAA; 8]);
        });
    }

    /// Buffers return to the pool, so sequential uses reuse one rather than
    /// allocating per call (the guest bump allocator never frees).
    #[test]
    #[cfg(feature = "alloc")]
    fn sequential_scratch_uses_reuse_one_buffer() {
        let first = with_scratch(|b| {
            b.reserve(crate::internal::SCRATCH_MAX);
            b.as_slice().as_ptr()
        });
        let second = with_scratch(|b| b.as_slice().as_ptr());
        assert_eq!(
            first, second,
            "scratch buffer was not returned to the pool"
        );
    }
}

#[inline]
fn fill_return_buf(
    return_len: u32,
    auth_token_id_list_return_len: u32,
    return_buf: &mut ReturnBuf,
    auth_token_id_list_buf: &mut AuthHandleBuf,
) -> Result<(), FaultString> {
    let mut written_mu: core::mem::MaybeUninit<RuntimeMemoryLength> =
        core::mem::MaybeUninit::uninit();
    let mut auth_token_id_list_written_mu: core::mem::MaybeUninit<
        RuntimeMemoryLength,
    > = core::mem::MaybeUninit::uninit();

    let auth_token_id_list_bytes =
        auth_handles_as_mut_bytes(auth_token_id_list_buf);

    sys::uninit_GetReturnedBuffer(
        return_buf.spare_capacity_mut(),
        &mut written_mu,
        auth_token_id_list_bytes,
        &mut auth_token_id_list_written_mu,
    )
    .map_err(|syserr| FaultString::from_display(format_args!("{syserr:?}")))?;

    let written = unsafe { written_mu.assume_init() };

    let written =
        checked_returned_len(written, return_len, return_buf.capacity())?;

    // Safety: `checked_returned_len` proved `written <= capacity`, so `set_len`
    // stays within the allocation. That the host actually initialized those
    // `written` bytes is trusted from the `GetReturnedBuffer` syscall contract,
    // not proven here; if the host under-fills, bytes in `[actual, written)`
    // would be read as valid data.
    unsafe {
        return_buf.set_len(written);
    }

    validate_auth_token_id_list_len(
        unsafe { auth_token_id_list_written_mu.assume_init() },
        auth_token_id_list_return_len,
    )?;

    Ok(())
}

/// Validate the length the host reports for a returned buffer before it is used
/// as the buffer's logical length.
///
/// `GetReturnedBuffer` copies at most `capacity` bytes into the destination
/// span but reports the *full* source length, which can exceed `capacity` when
/// a child returns a buffer larger than this fixed-size [`ReturnBuf`]. Treating
/// that reported length as the number of initialized bytes would push the
/// buffer's logical length past its capacity, so it is rejected as a parse
/// error here before any `set_len`.
fn checked_returned_len(
    written: RuntimeMemoryLength,
    return_len: u32,
    capacity: usize,
) -> Result<usize, FaultString> {
    if written != return_len {
        return Err(FaultString::from_display(format_args!(
            "unexpected return-data length from system call: \
                    written = {written}, \
                    return_len = {return_len}"
        )));
    }

    let written = written as usize;
    if written > capacity {
        return Err(FaultString::from_display(format_args!(
            "return-data length {written} exceeds return buffer \
                    capacity {capacity}"
        )));
    }

    Ok(written)
}

fn validate_auth_token_id_list_len(
    written_auth_token_id_list: RuntimeMemoryLength,
    auth_token_id_list_return_len: u32,
) -> Result<(), FaultString> {
    if written_auth_token_id_list != auth_token_id_list_return_len {
        return Err(FaultString::from_display(format_args!(
            "unexpected auth token id list length from system call: \
                    written = {written_auth_token_id_list}, \
                    auth_token_id_list_return_len = {auth_token_id_list_return_len}"
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::checked_returned_len;

    const CAPACITY: usize = 500;

    #[test]
    fn oversized_return_len_is_rejected() {
        // A child can return up to the protocol's 1024-byte cap. When it returns
        // 600 bytes, both `Invoke` and `GetReturnedBuffer` report 600 even though
        // only `CAPACITY` bytes fit in the destination span. Accepting 600 as the
        // logical length would push `ReturnBuf` past its capacity, so it must be
        // rejected.
        assert!(checked_returned_len(600, 600, CAPACITY).is_err());
    }

    #[test]
    fn return_len_at_capacity_is_accepted() {
        assert_eq!(
            checked_returned_len(CAPACITY as u32, CAPACITY as u32, CAPACITY)
                .unwrap(),
            CAPACITY
        );
    }

    #[test]
    fn return_len_within_capacity_is_accepted() {
        assert_eq!(checked_returned_len(480, 480, CAPACITY).unwrap(), 480);
    }

    #[test]
    fn empty_return_is_accepted() {
        assert_eq!(checked_returned_len(0, 0, CAPACITY).unwrap(), 0);
    }

    #[test]
    fn mismatched_written_and_return_len_is_rejected() {
        assert!(checked_returned_len(400, 500, CAPACITY).is_err());
    }
}
