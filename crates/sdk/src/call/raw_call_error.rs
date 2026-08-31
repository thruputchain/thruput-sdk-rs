//! The raw, untyped error from a contract call.

use crate::exn::{
    CallingError, FaultString, RawException,
    client_library_throw as library_throw,
};

/// The raw, untyped error from a contract call.
///
/// Generic over the exception representation `X`, which
/// [`try_call_raw_with`](super::try_call_raw_with)'s `on_exn` closure chooses:
/// the default [`RawException`] (one copy, via `to_owned`), a typed error
/// enum, or `Infallible` if the closure re-throws unconditionally.
#[derive(Debug, PartialEq)]
pub enum RawCallError<X = RawException> {
    /// The invoke syscall reported invalid parameters
    Invalid,
    /// The called contract threw an exception with a selector-tagged error.
    Exception(X),
    /// The called contract terminated without providing a return value.
    Fault(FaultString),
    /// Decode error: the called contract returned some unparseable return result
    UnparseableReturn(FaultString),
    /// Decode error: the called contract aborted with some unparseable result
    UnparseableAbort(FaultString),
    /// Decode error: the called contract aborted with some unparseable result
    UnparseableAuthHandles(FaultString),

    /// Encode error: the arguments supplied failed to encode
    EncodingArgs(FaultString),
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

/// A call failure with no exception payload: the target never ran
/// ([`Invalid`](RawCallError::Invalid)), faulted, or the host's returned
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
            RawCallError::Exception(exn) => {
                crate::re_throw::ReThrow::re_throw(exn)
            }
            // The message is host-side state, so only the host has one to
            // render. A guest carries no string for this: the rv32
            // `#[panic_handler]` ignores `PanicInfo` and issues `ebreak`,
            // so anything formatted here would be payload nothing reads.
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
}
