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
use thiserror::Error;

/// The result of a contract call, where `E` is the error-type specific to the
/// contract or method.
///
pub type CallResult<T = (), E = Infallible> = Result<T, CallError<E>>;

/// An error resulting from a contract call, where `E` is the error-type
/// specific to the contract or method.
///
#[derive(PartialEq, Error)]
pub enum CallError<E>
where
    E: Exn,
{
    /// The called contract requested abort.
    ///
    #[error("The called contract aborted")]
    Abort(#[from] CrossExn<E>),

    /// The called contract terminated without providing a return value.
    ///
    /// When `cfg(feature = "debugging")`, this contains a string describing
    /// the fault (e.g., what was passed to `panic`).
    ///
    #[error("The called contract faulted: {0}")]
    Fault(FaultString),
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
