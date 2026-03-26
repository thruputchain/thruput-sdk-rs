//! The type returned by the `try_*` methods generated
//! by the `#[contract]` macro.
//!
//! This type specifies the contract-specific types for success (`T`) and
//! abort (`E`), and can be derived via deserialization from `ContractResult`
//! if those types are known.

use crate::call::internal::contract_call_decode;
use crate::call::{ContractError, ContractResult};
use crate::exn::{CallingError, CrossExn, Exn, FaultString, library_abort};
use crate::xdr::*;
use core::convert::Infallible;
use core::fmt::{self, Debug};
#[cfg(feature = "debugging")]
use thiserror::Error;
/// The result of a contract call, where `E` is the error-type specific to the
/// contract or method.
///
pub type CallResult<T = (), E = Infallible> = Result<T, CallError<E>>;

/// An error resulting from a contract call, where `E` is the error-type
/// specific to the contract or method.
///
#[derive(PartialEq)]
#[cfg_attr(feature = "debugging", derive(Error))]
pub enum CallError<E>
where
    E: Exn,
{
    /// The called contract requested abort.
    ///
    #[cfg_attr(feature = "debugging", error("The called contract aborted"))]
    Abort(CrossExn<E>),

    /// The called contract terminated without providing a return value.
    ///
    /// When `cfg(feature = "debugging")`, this contains a string describing
    /// the fault (e.g., what was passed to `panic`).
    ///
    #[cfg_attr(
        feature = "debugging",
        error("The called contract faulted: {0}")
    )]
    Fault(FaultString),
}

impl<E> From<CrossExn<E>> for CallError<E>
where
    E: Exn,
{
    fn from(e: CrossExn<E>) -> Self {
        Self::Abort(e)
    }
}

impl<E> Debug for CallError<E>
where
    E: Exn,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Abort(exn) => {
                write!(f, "The called contract aborted \n{:?}", exn)
            }
            Self::Fault(fault) => {
                write!(f, "The called contract faulted: {}", fault)
            }
        }
    }
}

impl<E> CallError<E>
where
    E: Exn,
{
    pub fn into_fault_string(self) -> FaultString {
        FaultString::via_display(&self)
    }

    // Decode a contract error into a CallError.
    //
    // Decoding failure results in abort.
    //
    pub fn from_contract_error(e: ContractError) -> Self {
        match e {
            ContractError::Abort(abort_buf) => {
                // Decode into `CrossExn` and wrap in `CallError::Abort`.
                contract_call_decode(abort_buf.as_slice())
                        .map(CallError::Abort)
                        .unwrap_or_else(|e|
                            // Decode failure: abort ourselves
                            library_abort(CallingError::DecodingAbort(FaultString::from_display(e))))
            }
            ContractError::Fault(fault_val) => CallError::Fault(fault_val),
        }
    }
}

/// Decode a `ContractResult` into a typed `CallResult`, given the
/// caller-supplied success and failure types.
///
/// Decoding failure results in abort.
///
pub fn into_call_result<T, E>(
    contract_result: ContractResult,
) -> CallResult<T, E>
where
    T: ReadXDROwned,
    E: Exn,
{
    match contract_result {
        Ok(return_val) => {
            // Deserialize the return value
            contract_call_decode(return_val.as_slice())
                .map(Ok)
                .unwrap_or_else(|e|
                    // Decoding failure: abort ourselves
                    library_abort(CallingError::DecodingSuccess(FaultString::from_display(e))))
        }
        Err(err) => Err(CallError::from_contract_error(err)),
    }
}

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
    contract_result: ContractResult,
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
