//! The raw, untyped error from a contract call.

use crate::call::internal::contract_call_decode;
use crate::call::ContractError;
use crate::exn::{CallingError, FaultString, library_abort};
use crate::xdr::*;

/// The raw, untyped error from a contract call.
#[derive(Debug, PartialEq)]
pub enum RawCallError {
    /// The called contract aborted with a selector-tagged error.
    Abort {
        /// The u64 selector identifying the error type.
        selector: u64,
        /// The raw bytes after the selector.
        payload: heapless::Vec<u8, 500>,
    },
    /// The called contract terminated without providing a return value.
    Fault(FaultString),
}

/// Decode a `ContractResult` into a `Result<T, RawCallError>`, preserving the
/// raw selector and payload from an abort without attempting typed decoding.
///
/// Decoding failure on the success path results in abort.
///
pub fn into_raw_call_result<T>(
    contract_result: crate::call::ContractResult,
) -> Result<T, RawCallError>
where
    T: ReadXDROwned,
{
    match contract_result {
        Ok(return_val) => {
            contract_call_decode(return_val.as_slice())
                .map(Ok)
                .unwrap_or_else(|e|
                    library_abort(CallingError::DecodingSuccess(FaultString::from_display(e))))
        }
        Err(ContractError::Abort(abort_buf)) => {
            let bytes = abort_buf.as_slice();
            if bytes.len() < 8 {
                library_abort(CallingError::DecodingAbort(
                    FaultString::from_display("abort buffer too short for selector")))
            }

            let selector = u64::from_be_bytes(bytes[..8].try_into().unwrap());
            let payload_bytes = &bytes[8..];

            let mut payload = heapless::Vec::new();
            payload.extend_from_slice(payload_bytes).ok();

            Err(RawCallError::Abort { selector, payload })
        }
        Err(ContractError::Fault(fault_val)) => Err(RawCallError::Fault(fault_val)),
    }
}
