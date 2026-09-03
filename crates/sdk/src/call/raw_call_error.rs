//! The raw, untyped error from a contract call.

use crate::exn::{
    CallingError, FaultInfo, FaultString, RawException,
    client_library_throw as library_throw, syscall_exn,
};
use crate::sys::{SysErr, SyscallNo};

/// The raw, untyped error from a contract call.
///
/// Generic over the exception representation `X`, which
/// [`try_call_raw_with`](super::try_call_raw_with)'s `on_exn` closure chooses:
/// the default [`RawException`] (one copy, via `to_owned`), a typed error
/// enum, or `Infallible` if the closure re-throws unconditionally.
///
/// The variants divide by how far the call got.
/// [`Invalid`](Self::Invalid), [`Reentrance`](Self::Reentrance),
/// [`DepthExceeded`](Self::DepthExceeded) and
/// [`InvokeRejected`](Self::InvokeRejected) all mean the invoke syscall was
/// refused before any child frame ran, so the call was a complete no-op.
/// [`Exception`](Self::Exception) and [`Fault`](Self::Fault) mean a child
/// frame ran and then aborted or died. The rest are encode/decode failures
/// in this contract, on either side of a call that otherwise completed.
#[derive(Debug, PartialEq)]
pub enum RawCallError<X = RawException> {
    /// The invoke syscall rejected the call as malformed: no contract at the
    /// target address, invalid invoke flags, or an auth token id that is
    /// malformed, duplicated, or not owned by this frame.
    Invalid,

    /// The target address already has a frame open on the invocation stack,
    /// so the runtime refused the call as reentrant.
    ///
    /// The call can never succeed anywhere in this call chain — unlike
    /// [`DepthExceeded`](Self::DepthExceeded), retrying from a shallower
    /// frame does not help while this address is still on the stack.
    Reentrance,

    /// The transaction already holds the network's maximum number of open
    /// invocation frames, so admitting another would exceed the cross-invoke
    /// depth bound.
    ///
    /// A resource limit rather than a malformed call: the same invocation
    /// succeeds when made from a shallower frame.
    DepthExceeded,

    /// The invoke syscall was refused with a code this SDK does not model.
    ///
    /// Carries the raw code rather than collapsing an unrecognized rejection
    /// into a neighbouring variant's meaning.
    InvokeRejected(SysErr),

    /// The called contract threw an exception with a selector-tagged error.
    Exception(X),

    /// A child frame ran and terminated without producing a result: a panic,
    /// a trap, or exhausting its gas.
    ///
    /// The message is *host-side state recorded beside the call*, never
    /// something the callee sent: the runtime reports a faulted child as a
    /// bare frame status with neither a return nor an abort buffer, so no
    /// message crosses the call boundary and none ever will. That splits the
    /// two cases:
    ///
    /// - **A frame the native harness ran itself** — the root frame of an
    ///   `env.transact`, reaching here through `into_root_call_result`. The
    ///   harness caught that panic, so the message is the panicking body's
    ///   own.
    /// - **A `raw_Invoke`ed callee** — caught inside the runtime's own
    ///   frame, where there is nothing to attribute a message against, so
    ///   the payload is [`FaultInfo::empty`] and `as_str()` reads
    ///   `"<fault>"`. Reporting anything else here would mean reporting some
    ///   other frame's message.
    ///
    /// [`FaultInfo`] carries the panic's location alongside its message
    /// where the harness saw one; `Display` renders them as
    /// `message at file:line:column`.
    ///
    /// On rv32 there is nothing either way: [`FaultInfo`] is a zero-sized
    /// type there, so a guest pays nothing for carrying one.
    ///
    /// A callee that wants its failure to be legible to its caller — on
    /// chain, not just in a host test — must abort with an exception rather
    /// than fault.
    Fault(FaultInfo),

    /// Decode error: the called contract returned some unparseable return result
    UnparseableReturn(FaultString),
    /// Decode error: the called contract aborted with some unparseable result
    UnparseableAbort(FaultString),
    /// Decode error: the called contract aborted with some unparseable result
    UnparseableAuthHandles(FaultString),

    /// Encode error: the arguments supplied failed to encode
    EncodingArgs(FaultString),
}

impl<X> RawCallError<X> {
    /// Whether the invoke syscall was refused before any child frame ran.
    ///
    /// True for [`Invalid`](Self::Invalid), [`Reentrance`](Self::Reentrance),
    /// [`DepthExceeded`](Self::DepthExceeded) and
    /// [`InvokeRejected`](Self::InvokeRejected). When this holds the call was
    /// a complete no-op: the callee did not run, observed nothing, and wrote
    /// nothing.
    pub fn is_rejected_before_call(&self) -> bool {
        matches!(
            self,
            RawCallError::Invalid
                | RawCallError::Reentrance
                | RawCallError::DepthExceeded
                | RawCallError::InvokeRejected(_)
        )
    }
}

/// Classify the `Err` from the Invoke syscall, which the runtime returns only
/// when it refused the call before running a child frame.
///
/// The codes are those `Invoke` documents in the syscall spec; anything else
/// keeps its raw code in [`RawCallError::InvokeRejected`] rather than being
/// folded into one of the modelled meanings.
pub(crate) fn classify_invoke_rejection<X>(syserr: SysErr) -> RawCallError<X> {
    use crate::sys::SyscallErrorCode as Code;

    if syserr.is_invalid_input() {
        RawCallError::Invalid
    } else if syserr.is_code(Code::kSyscallErrorCodeReentrance) {
        RawCallError::Reentrance
    } else if syserr.is_code(Code::kSyscallErrorCodeInvokeDepthExceeded) {
        RawCallError::DepthExceeded
    } else {
        RawCallError::InvokeRejected(syserr)
    }
}

/// The error type of every generated `try_*` client method: a call failure
/// that may carry a caught exception, kept untyped.
///
/// Exceptions stay as [`RawException`] rather than being pre-decoded into a
/// per-client enum, because the wire form already identifies the type by
/// selector, independently of what any interface declared it `throws`. Match
/// one with [`match_exn!`](crate::match_exn); everything else here is
/// ordinary `match` material.
pub type CallError = RawCallError;

/// A call failure with no exception payload: the call was refused before the
/// target ran (see [`is_rejected_before_call`](RawCallError::is_rejected_before_call)),
/// the target faulted, or the host's returned
/// data failed validation. The `Exception` variant is statically
/// uninhabited ([`Infallible`](core::convert::Infallible)) — on these
/// paths an abort is not an error; it surfaces as
/// [`CallOutcome::Abort`](crate::call::CallOutcome::Abort) with the
/// payload in the caller's buffer. (Crate-internal code widens into a
/// `RawCallError<X>` with `widen_uninhabited_exn`.)
pub type CallFault = RawCallError<core::convert::Infallible>;

#[cfg(not(target_arch = "riscv32"))]
impl<X: core::fmt::Debug> core::fmt::Display for RawCallError<X> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            RawCallError::Invalid => {
                write!(f, "contract not found (or invalid auth tokens)")
            }
            RawCallError::Reentrance => {
                write!(f, "call refused: target already has an open frame")
            }
            RawCallError::DepthExceeded => {
                write!(f, "call refused: cross-invoke depth bound reached")
            }
            RawCallError::InvokeRejected(code) => {
                write!(f, "call refused: {code:?}")
            }
            RawCallError::Exception(exn) => write!(f, "exception: {exn:?}"),
            RawCallError::Fault(fault) => write!(f, "fault: {fault}"),
            RawCallError::UnparseableReturn(error) => {
                write!(f, "unparseable return: {error}")
            }
            RawCallError::UnparseableAbort(error) => {
                write!(f, "unparseable abort: {error}")
            }
            RawCallError::UnparseableAuthHandles(error) => {
                write!(f, "unparseable auth handles: {error}")
            }
            RawCallError::EncodingArgs(error) => {
                write!(f, "encoding args: {error}")
            }
        }
    }
}

impl<X: crate::re_throw::ReThrow> crate::re_throw::ReThrow
    for RawCallError<X>
{
    #[cfg_attr(not(target_arch = "riscv32"), track_caller)]
    fn re_throw(self) -> ! {
        match self {
            RawCallError::Invalid => library_throw(CallingError::Invalid),
            RawCallError::Reentrance => {
                library_throw(CallingError::Reentrance)
            }
            RawCallError::DepthExceeded => {
                library_throw(CallingError::DepthExceeded)
            }
            // An unmodelled rejection keeps its raw code by riding the
            // existing syscall exception rather than earning a `CallingError`
            // variant of its own: there is no meaning to name yet, and the
            // code is what a caller would want to see.
            RawCallError::InvokeRejected(code) => {
                library_throw(syscall_exn(SyscallNo::kInvoke, code))
            }
            RawCallError::Exception(exn) => {
                crate::re_throw::ReThrow::re_throw(exn)
            }
            // Renders whatever host-side description was recorded beside
            // the call, or `"<fault>"` when none was. A guest has no message
            // to render and no way to surface one: its `#[panic_handler]`
            // ignores `PanicInfo` and issues `ebreak`.
            #[cfg(not(target_arch = "riscv32"))]
            RawCallError::Fault(f) => panic!("{f}"),
            #[cfg(target_arch = "riscv32")]
            RawCallError::Fault(_) => panic!(),
            RawCallError::UnparseableReturn(error) => {
                library_throw(CallingError::DecodingSuccess(error))
            }
            RawCallError::UnparseableAbort(error) => {
                library_throw(CallingError::DecodingException(error))
            }
            RawCallError::UnparseableAuthHandles(error) => {
                library_throw(CallingError::DecodingSuccess(error))
            }
            RawCallError::EncodingArgs(error) => {
                library_throw(CallingError::EncodingArgs(error))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::re_throw::OrReThrow;

    #[test]
    fn or_re_throw_ok_returns_value() {
        let result: Result<i32, RawCallError> = Ok(42);
        assert_eq!(result.or_re_throw(), 42);
    }

    /// Every rejection `Invoke` documents keeps its own identity. Reentrance
    /// and the depth bound used to land in `Fault`, which told the caller its
    /// callee had misbehaved when in fact the callee never ran — and on rv32,
    /// where the old `Fault` payload compiled out, nothing distinguished them
    /// at all.
    #[test]
    fn documented_rejections_map_to_their_own_variants() {
        use crate::sys::SyscallErrorCode as Code;

        let cases = [
            (
                Code::kSyscallErrorCodeInvalidSyscallInput,
                RawCallError::Invalid,
            ),
            (Code::kSyscallErrorCodeReentrance, RawCallError::Reentrance),
            (
                Code::kSyscallErrorCodeInvokeDepthExceeded,
                RawCallError::DepthExceeded,
            ),
        ];

        for (code, expected) in cases {
            let got: RawCallError = classify_invoke_rejection(code.into());
            assert_eq!(got, expected, "rejection code {code:?}");
        }
    }

    /// A rejection this SDK does not model keeps its raw code instead of
    /// being folded into a neighbouring variant's meaning.
    #[test]
    fn unmodelled_rejection_keeps_its_code() {
        let unknown = SysErr(0x7fff_0000);
        let got: RawCallError = classify_invoke_rejection(unknown);
        assert_eq!(got, RawCallError::InvokeRejected(unknown));
    }

    /// `Fault` means a child frame ran and died; every rejection means it
    /// never ran at all. Contracts that must not retry a call whose side
    /// effects may have landed depend on that split.
    #[test]
    fn rejections_are_distinguished_from_a_ran_and_faulted_callee() {
        let rejected: [RawCallError; 4] = [
            RawCallError::Invalid,
            RawCallError::Reentrance,
            RawCallError::DepthExceeded,
            RawCallError::InvokeRejected(SysErr(1234)),
        ];
        for e in &rejected {
            assert!(e.is_rejected_before_call(), "{e:?}");
            assert_ne!(*e, RawCallError::Fault(FaultInfo::empty()));
        }

        let ran: RawCallError = RawCallError::Fault(FaultInfo::empty());
        assert!(!ran.is_rejected_before_call());
    }
}
