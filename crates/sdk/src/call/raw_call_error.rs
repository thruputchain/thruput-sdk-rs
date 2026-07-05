//! The raw, untyped error from a contract call.

#[cfg(feature = "alloc")]
use super::ContractError;
#[cfg(feature = "alloc")]
use super::contract_call_decode;
use crate::exn::{
    CallingError, FaultString, RawException,
    client_library_throw as library_throw,
};
#[cfg(feature = "alloc")]
use crate::{ReceiveAuthHandles, xdr::*};

/// The raw, untyped error from a contract call.
#[derive(Debug, PartialEq)]
pub enum RawCallError {
    /// The invoke syscall reported invalid parameters
    Invalid,
    /// The called contract threw an exception with a selector-tagged error.
    Exception(RawException),
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

#[cfg(feature = "debugging")]
impl core::fmt::Display for RawCallError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            RawCallError::Invalid => {
                write!(f, "contract not found (or invalid auth tokens)")
            }
            RawCallError::Exception(exn) => exn.with_payload(|payload| {
                write!(
                    f,
                    "exception (selector=0x{:016x}, payload={})",
                    exn.selector(),
                    crate::hex::hex_encode(&payload)
                )
            }),
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

impl crate::re_throw::ReThrow for RawCallError {
    #[cfg_attr(feature = "debugging", track_caller)]
    fn re_throw(self) -> ! {
        match self {
            RawCallError::Invalid => library_throw(CallingError::Invalid),
            RawCallError::Exception(exn) => {
                crate::re_throw::ReThrow::re_throw(exn)
            }
            RawCallError::Fault(f) => panic!("{f}"),
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

/// Decode a `ContractResult` into a `Result<T, RawCallError>`, preserving the
/// raw selector and payload from an exception without attempting typed decoding.
///
/// Decoding failure on the success path results in an exception.
///
/// `alloc`-only mock-side plumbing (builds a `RawException` from an owned
/// abort buffer, which the no-alloc guest never does).
#[cfg(feature = "alloc")]
pub fn into_raw_call_result<T>(
    contract_result: crate::call::ContractResult,
) -> Result<T, RawCallError>
where
    T: ReadXDROwned + ReceiveAuthHandles,
{
    match contract_result {
        Ok(return_val) => contract_call_decode(
            return_val.0.as_slice(),
            return_val.1.as_slice(),
        )
        .map(Ok)
        .unwrap_or_else(|e| {
            Err(RawCallError::UnparseableReturn(FaultString::from_display(
                e,
            )))
        }),
        Err(ContractError::Abort(abort_buf)) => {
            match RawException::from_abort_buf(abort_buf.as_slice()) {
                Ok(exn) => Err(RawCallError::Exception(exn)),
                Err(e) => Err(RawCallError::UnparseableAbort(
                    FaultString::from_display(e),
                )),
            }
        }
        Err(ContractError::Fault(fault_val)) => {
            Err(RawCallError::Fault(fault_val))
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
