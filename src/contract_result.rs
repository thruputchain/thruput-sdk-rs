//! An error type convenient for contracts, in that it can represent many typical errors

use crate::result::ErrorMsg;
use crate::serialize::{Deserialize, Serialize};
#[cfg(feature = "storage")]
use crate::storage::error::Error as StorageErr;
use crate::syscall::SysErr;
use crate::invoke_error::InvokeError;
use ::thiserror::Error;
use ::xdrs::{ReadXDR, WriteXDR};

/// Contract result
pub type Result<T, E = Error> = core::result::Result<T, E>;

/// Contract error
#[derive(Error, Debug, Clone, PartialEq, Serialize, Deserialize, ReadXDR, WriteXDR)]
#[repr(i32)]
pub enum Error {
    /// The caller passed in erroneous data
    #[error("Caller error{0}")]
    Caller(ErrorMsg) = ::groundhog_xdr::contract_error::ErrorType::kErrorTypeCaller as i32,

    /// The contract experienced an internal failure
    #[error("Contract internal error{0}")]
    Contract(#[from] ContractError) = ::groundhog_xdr::contract_error::ErrorType::kErrorTypeContract as i32,
}

/// An error internal to a contract
#[derive(Error, Debug, Clone, PartialEq, Serialize, Deserialize, ReadXDR, WriteXDR)]
#[repr(i32)]
pub enum ContractError {
    /// A custom error message
    #[error(": {0}")]
    Custom(ErrorMsg) = ::groundhog_xdr::contract_error::ContractErrorType::kContractErrorTypeCustom as i32,

    /// Failed syscall
    #[error(": failed due to syscall error: {0}")]
    Syscall(#[from] SysErr) = ::groundhog_xdr::contract_error::ContractErrorType::kContractErrorTypeSyscall as i32,

    // TODO:put the InvokeError here
    /// Failed call of another contract
    #[error(": failed due to cross-contract call failing or aborting")]
    Call = groundhog_xdr::contract_error::ContractErrorType::kContractErrorTypeCall as i32,

    /// Failure from the storage layer
    #[cfg(feature = "storage")]
    #[error(": failed due to storage error: {0}")]
    Storage(#[from] StorageErr) = ::groundhog_xdr::contract_error::ContractErrorType::kContractErrorTypeStorage as i32,
}

impl From<SysErr> for Error {
    fn from(err: SysErr) -> Self {
        Error::Contract(err.into())
    }
}

impl From<InvokeError> for Error {
    fn from(_err: InvokeError) -> Self {
        Error::Contract(ContractError::Call)
    }
}

#[cfg(feature = "storage")]
impl From<StorageErr> for Error {
    fn from(err: StorageErr) -> Self {
        Error::Contract(err.into())
    }
}

impl Error {
    pub fn is_caller_err(&self) -> bool {
        matches!(self, Error::Caller(_))
    }

    pub fn is_contract_err(&self) -> bool {
        matches!(self, Error::Contract(_))
    }
}

/// Create an internal error from an arbitrary message
pub fn internal_err<T, M: AsRef<str>>(msg: M) -> Result<T> {
    let msg = ErrorMsg::new(msg.as_ref());
    let contract_err = ContractError::Custom(msg);
    let error = Error::Contract(contract_err);

    Err(error)
}

/// Create a caller error from an arbitrary message
pub fn caller_error<T, M: AsRef<str>>(msg: M) -> Result<T> {
    Err(Error::Caller(ErrorMsg::new(msg.as_ref())))
}

pub fn caller_ensure<M: AsRef<str>>(pred: bool, msg: M) -> Result<()> {
    if pred {
        Ok(())
    } else {
        caller_error(msg)
    }
}
