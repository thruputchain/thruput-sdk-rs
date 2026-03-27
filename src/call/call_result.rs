//! The raw, untyped error from a contract call.

use crate::call::ContractError;
use crate::call::internal::contract_call_decode;
use crate::exn::{CallingError, FaultString, library_throw};
use crate::xdr::*;

/// The raw, untyped error from a contract call.
#[derive(Debug, PartialEq)]
pub enum RawCallError {
    /// The called contract threw an exception with a selector-tagged error.
    Exception {
        /// The u64 selector identifying the error type.
        selector: u64,
        /// The raw bytes after the selector.
        payload: heapless::Vec<u8, 500>,
    },
    /// The called contract terminated without providing a return value.
    Fault(FaultString),
}

#[cfg(feature = "debugging")]
impl core::fmt::Display for RawCallError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            RawCallError::Exception { selector, payload } => write!(
                f,
                "exception (selector=0x{:016x}, payload={})",
                selector,
                crate::hex::hex_encode(payload)
            ),
            RawCallError::Fault(fault) => write!(f, "fault: {fault}"),
        }
    }
}

/// Decode a `ContractResult` into a `Result<T, RawCallError>`, preserving the
/// raw selector and payload from an exception without attempting typed decoding.
///
/// Decoding failure on the success path results in an exception.
///
pub fn into_raw_call_result<T>(
    contract_result: crate::call::ContractResult,
) -> Result<T, RawCallError>
where
    T: ReadXDROwned,
{
    match contract_result {
        Ok(return_val) => contract_call_decode(return_val.as_slice())
            .map(Ok)
            .unwrap_or_else(|e| {
                library_throw(CallingError::DecodingSuccess(
                    FaultString::from_display(e),
                ))
            }),
        Err(ContractError::Abort(abort_buf)) => {
            let bytes = abort_buf.as_slice();

            // The exception buffer is CXS-encoded: (u64_selector, payload...).
            // Decode the selector using a CXS reader so we consume the
            // correct number of bytes (CXS uses variable-length integer encoding).
            let mut reader = cxs::buf_reader::BufReader::new(bytes);
            let selector: u64 =
                ReadXDR::read_xdr(&mut reader).unwrap_or_else(|_| {
                    library_throw(CallingError::DecodingException(
                        FaultString::from_display(
                            "failed to decode selector from exception buffer",
                        ),
                    ))
                });
            let payload_bytes = &bytes[reader.position()..];

            let mut payload = heapless::Vec::new();
            payload.extend_from_slice(payload_bytes).ok();

            Err(RawCallError::Exception { selector, payload })
        }
        Err(ContractError::Fault(fault_val)) => {
            Err(RawCallError::Fault(fault_val))
        }
    }
}
