//! Making contract calls

use super::contract_result::*;
use super::encode::*;
use super::raw_call_error::*;
use crate::contract::ContractEntrypointID;
use crate::exn::{FaultString, RawException};
use crate::types::transaction::{
    InvocationFrameStatusCode, kMaxReturnBufferLen,
};
use crate::types::{ContractAddress, RuntimeMemoryLength};
use crate::xdr::*;
use crate::{AuthHandleBuf, sys};

/// Typed cross-contract call: encode `args`, invoke, decode the return
/// value (or surface the abort as a [`RawCallError`]).
///
/// This is what generated clients use. Both the encoded-args and
/// returned-data bytes live in a lazily-allocated per-instance scratch
/// reused across calls (under `alloc`), so repeated calls perform no
/// per-call allocations even though the guest bump allocator never frees,
/// and neither outgoing calldata nor received returns have an SDK-imposed
/// size cap.
// Used by the contract macro in method wrappers
#[cfg_attr(feature = "debugging", track_caller)]
pub fn try_call_raw<ARGS, RET>(
    address: &ContractAddress,
    entrypoint: ContractEntrypointID,
    gas_limit: u64,
    args: &ARGS,
    flags: crate::invoke_config::InvokeFlagSet,
) -> Result<RET, RawCallError>
where
    ARGS: WriteXDR + crate::CollectAuthHandles,
    RET: ReadXDROwned + crate::ReceiveAuthHandles,
{
    // The scratch data buffer does double duty within one call: it holds
    // the encoded args while `Invoke` copies them out, then is cleared and
    // refilled with the returned data. Likewise the auth buffer holds the
    // outgoing handle list during `Invoke`, then the returned one. The
    // decoded RET is owned; a no-alloc RawException's payload stays behind
    // in the scratch (referenced by generation), which is fine because the
    // borrow ends with this closure and the next checkout invalidates it.
    with_call_scratch(|buf, auth_buf, scratch_generation| {
        encode_args_into(buf, args)?;
        auth_buf.clear();
        args.collect_auth_handles(auth_buf)
            .map_err(RawCallError::EncodingArgs)?;

        let (is_ok, outputs) = invoke_frame(
            address,
            entrypoint,
            gas_limit,
            buf.as_slice(),
            auth_buf,
            flags,
        )?;

        buf.clear();
        resize_auth_handles(auth_buf, &outputs)?;
        fill_return_buf(
            outputs.return_buffer_len,
            outputs.returned_auth_token_len,
            buf,
            auth_buf,
        )
        .map_err(|fault| unparseable(is_ok, fault))?;

        if is_ok {
            contract_call_decode(buf.as_slice(), auth_buf.as_slice())
        } else {
            match RawException::from_scratch_abort(
                buf.as_slice(),
                scratch_generation,
            ) {
                Ok(exn) => Err(RawCallError::Exception(exn)),
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
/// fixed-capacity buffer rejects encodings larger than 500 bytes.
fn encode_args_into<T: WriteXDR + ?Sized>(
    buf: &mut Buf,
    value: &T,
) -> Result<(), RawCallError> {
    buf.clear();
    #[cfg(feature = "alloc")]
    {
        let mut writer =
            cxs::buf_writer::BufWriter::from(core::mem::take(buf));
        let result = value.write_xdr(&mut writer);
        *buf = writer.into();
        result.map_err(|e| {
            RawCallError::EncodingArgs(FaultString::from_display(e))
        })
    }
    #[cfg(not(feature = "alloc"))]
    {
        let written = cxs::encode_into(value, buf.spare_capacity_mut())
            .map_err(|e| {
                RawCallError::EncodingArgs(FaultString::from_display(e))
            })?;
        // Safety: `encode_into` initialized exactly `written` bytes.
        unsafe { buf.set_len(written) };
        Ok(())
    }
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
/// in `returned_auth_token_ids` (both cleared first).
///
/// Prefer [`try_call_raw`] (or a generated client) for ordinary typed
/// calls. This entry point exists for forwarders and proxies — e.g. wallet
/// contracts that relay opaque calldata to a target and pass the raw
/// return/abort buffer back verbatim — which cannot name the argument or
/// return types. The payload lands in the caller's buffers rather than
/// being returned by value: the no-alloc buffer is `SCRATCH_MAX` bytes,
/// and a by-value return would copy it once per call layer.
pub fn contract_call<A>(
    address: &ContractAddress,
    entrypoint: ContractEntrypointID,
    gas_limit: u64,
    args: &A,
    auth_token_ids: &crate::collect_auth_handles::AuthHandleBuf,
    flags: crate::invoke_config::InvokeFlagSet,
    return_buf: &mut ReturnBuf,
    returned_auth_token_ids: &mut AuthHandleBuf,
) -> Result<CallOutcome, RawCallError>
where
    A: AsRef<[u8]> + ?Sized,
{
    let (is_ok, outputs) = invoke_frame(
        address,
        entrypoint,
        gas_limit,
        args.as_ref(),
        auth_token_ids,
        flags,
    )?;

    return_buf.clear();
    #[cfg(feature = "alloc")]
    return_buf.reserve(outputs.return_buffer_len as usize);

    resize_auth_handles(returned_auth_token_ids, &outputs)?;
    fill_return_buf(
        outputs.return_buffer_len,
        outputs.returned_auth_token_len,
        return_buf,
        returned_auth_token_ids,
    )
    .map_err(|fault| unparseable(is_ok, fault))?;

    Ok(if is_ok {
        CallOutcome::Return
    } else {
        CallOutcome::Abort
    })
}

/// Map a returned-data parse fault to the error variant matching the
/// frame's outcome (success vs abort).
fn unparseable(is_ok: bool, fault: FaultString) -> RawCallError {
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
) -> Result<(bool, sys::CopyableInvokeOutputs), RawCallError> {
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
    let result =
        sys::raw_Invoke(address, args, auth_bytes, &inputs, &mut outputs);

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
fn resize_auth_handles(
    buf: &mut AuthHandleBuf,
    outputs: &sys::CopyableInvokeOutputs,
) -> Result<(), RawCallError> {
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

use crate::cell::{ContractLocalRef, ContractRefCell};

/// The per-instance call scratch: the shared write buffer, the auth-handle
/// list, and a checkout generation. The generation increments on every
/// checkout; a no-alloc [`RawException`] records it so its payload (left
/// sitting in `buf` by the aborting call) can be read back zero-copy and a
/// stale read — after a later scratch use overwrote the payload — traps
/// instead of returning the wrong bytes.
struct CallScratch {
    generation: u64,
    buf: Buf,
    auth: AuthHandleBuf,
}

/// The generation passed to fallback (fresh-buffer) checkouts. Never equals
/// a cell generation (those start at 1), so a `RawException` minted against
/// a fallback buffer is always-stale by construction.
const SCRATCH_GENERATION_INVALID: u64 = 0;

crate::contract_local! {
    static CALL_SCRATCH: ContractRefCell<CallScratch> =
        const {
            ContractRefCell::new(CallScratch {
                generation: SCRATCH_GENERATION_INVALID,
                buf: Buf::new(),
                auth: AuthHandleBuf::new(),
            })
        };
}

/// Run `f` with the per-instance call scratch buffers (encoded-args /
/// returned-data, and the auth-token-id list).
///
/// The scratch is the SDK's single shared write buffer: cross-contract
/// calls, event emission, storage writes, and terminal returns all encode
/// through it (calldata is the one separate buffer, since decoded args
/// borrow from it for the whole frame). It is allocated lazily and reused by
/// every subsequent use: the guest bump allocator never frees, so per-use
/// allocations would permanently leak heap. Under `alloc` capacity grows to
/// the high-water mark (at least `SCRATCH_MAX`). No user code runs while the
/// scratch is checked out, so uses cannot overlap; if the slot is somehow
/// unavailable (re-entrancy), a fresh default buffer is used and the result
/// is correct, just momentarily un-cached.
fn with_call_scratch<R>(
    f: impl FnOnce(&mut Buf, &mut AuthHandleBuf, u64) -> R,
) -> R {
    CALL_SCRATCH.with_ref(|cell| {
        match cell.try_borrow_mut() {
            Ok(mut guard) => {
                // Borrow the reused buffers in place — no copy, and for the
                // no-alloc guest the fixed buffer stays in static memory
                // instead of being moved onto the (small) guest stack.
                let scratch = &mut *guard;
                // Invalidate any RawException still referencing the
                // previous contents.
                scratch.generation += 1;
                scratch.buf.clear();
                scratch.auth.clear();
                #[cfg(feature = "alloc")]
                if scratch.buf.capacity() < crate::internal::SCRATCH_MAX {
                    scratch.buf.reserve(crate::internal::SCRATCH_MAX);
                }
                f(&mut scratch.buf, &mut scratch.auth, scratch.generation)
            }
            Err(_) => call_scratch_fallback(f),
        }
    })
}

/// Run `f` on the call-scratch buffer contents if `generation` is still the
/// cell's current checkout and `range` is in bounds; `None` if the scratch
/// has been reused since (or is currently checked out mutably). Zero-copy
/// read-back path for the no-alloc [`RawException`], whose payload the
/// aborting call left sitting in the scratch.
#[cfg(not(feature = "alloc"))]
pub(crate) fn with_scratch_payload_if_current<R>(
    generation: u64,
    range: core::ops::Range<usize>,
    f: impl FnOnce(&[u8]) -> R,
) -> Option<R> {
    CALL_SCRATCH.with_ref(|cell| match cell.try_borrow() {
        Ok(guard) if guard.generation == generation => {
            guard.buf.as_slice().get(range).map(f)
        }
        _ => None,
    })
}

/// Abort with the verbatim scratch contents `[..end]` — the original abort
/// wire bytes — if `generation` is still current. Returns (rather than
/// diverging) when the payload is stale, so the caller can trap with its
/// own diagnosis. The no-alloc re-throw path: no re-encode, no copy.
#[cfg(not(feature = "alloc"))]
pub(crate) fn reabort_scratch_if_current(generation: u64, end: usize) {
    CALL_SCRATCH.with_ref(|cell| {
        if let Ok(guard) = cell.try_borrow() {
            if guard.generation == generation {
                if let Some(bytes) = guard.buf.as_slice().get(..end) {
                    crate::contract_op::contract_abort(bytes);
                }
            }
        }
    })
}

/// Re-entrant use: the scratch is already borrowed up-stack — a cross-call
/// nested in-process (native mock), or an exception thrown while the
/// scratch is borrowed (the abort encode re-enters). Use fresh buffers so
/// the borrow can't alias; correct, just momentarily un-cached. (The real
/// guest doesn't nest cross-calls, but the throw-mid-encode case can happen
/// there, so this is not mock-only.) Out of line and `cold` so the fresh
/// (no-alloc: SCRATCH_MAX-byte) buffers don't get pre-allocated in every
/// caller's prologue when the happy path is inlined.
#[cold]
#[inline(never)]
fn call_scratch_fallback<R>(
    f: impl FnOnce(&mut Buf, &mut AuthHandleBuf, u64) -> R,
) -> R {
    let mut buf = crate::internal::new_scratch();
    let mut auth_buf = AuthHandleBuf::new();
    f(&mut buf, &mut auth_buf, SCRATCH_GENERATION_INVALID)
}

/// Run `f` with just the per-instance data scratch buffer. For
/// encode-then-syscall sites (events, storage values, terminal returns) that
/// don't touch the auth-handle list. Shares the cell with [`with_call_scratch`].
pub(crate) fn with_scratch<R>(f: impl FnOnce(&mut Buf) -> R) -> R {
    with_call_scratch(|buf, _auth, _generation| f(buf))
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
