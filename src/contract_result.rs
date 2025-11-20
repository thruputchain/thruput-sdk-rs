//! General-purpose result type for contracts

use crate::SysErr;
use crate::sysresult;
use crate::sysresult::ErrorMsg;
use thiserror::Error;
use xdrs::{ReadXDR, WriteXDR};

#[cfg(feature = "storage")]
use crate::storage::error::StorageError as StorageErr;

use crate::contract_error::{ContractErrorType, ErrorType};

#[cfg(feature = "spec")]
use xdrs::xdr_repr::XDR;

#[derive(Error, Debug, Clone, PartialEq, ReadXDR, WriteXDR)]
#[repr(i32)]
#[cfg_attr(feature = "spec", derive(xdrs::XDR))]
pub enum Error {
    #[error("Caller error: {msg}")]
    Caller { msg: ErrorMsg } = ErrorType::kErrorTypeCaller as i32,

    #[error("Contract internal error: {contract_error}")]
    Contract {
        #[from]
        contract_error: ContractError,
    } = ErrorType::kErrorTypeContract as i32,
}

#[derive(Error, Debug, Clone, PartialEq, ReadXDR, WriteXDR)]
#[repr(i32)]
#[cfg_attr(feature = "spec", derive(xdrs::XDR))]
pub enum ContractError {
    #[error("Message: {msg}")]
    Custom { msg: ErrorMsg } =
        ContractErrorType::kContractErrorTypeCustom as i32,

    #[error("Syscall failed: {syscall_error}")]
    Syscall {
        #[from]
        syscall_error: SysErr,
    } = ContractErrorType::kContractErrorTypeSyscall as i32,

    #[cfg(feature = "storage")]
    #[error("Storage failure: {storage_error}")]
    Storage {
        #[from]
        storage_error: StorageErr,
    } = ContractErrorType::kContractErrorTypeStorage as i32,
}

impl From<sysresult::Error> for Error {
    // TODO: rework error hierarchy
    fn from(_err: sysresult::Error) -> Self {
        Error::Contract {
            contract_error: ContractError::Custom {
                msg: ErrorMsg::new("library error"),
            },
        }
    }
}

impl From<SysErr> for Error {
    fn from(err: SysErr) -> Self {
        Error::Contract {
            contract_error: err.into(),
        }
    }
}

#[cfg(feature = "storage")]
impl From<StorageErr> for Error {
    fn from(err: StorageErr) -> Self {
        Error::Contract {
            contract_error: err.into(),
        }
    }
}

impl Error {
    pub fn is_caller_err(&self) -> bool {
        matches!(self, Error::Caller { msg: _ })
    }

    pub fn is_contract_err(&self) -> bool {
        matches!(self, Error::Contract { contract_error: _ })
    }
}

pub type Result<T, E = Error> = core::result::Result<T, E>;

pub fn internal_err<T, M: AsRef<str>>(msg: M) -> Result<T> {
    let msg = ErrorMsg::new(msg.as_ref());
    let contract_err = ContractError::Custom { msg };
    let error = Error::Contract {
        contract_error: contract_err,
    };

    Err(error)
}

pub fn caller_error<T, M: AsRef<str>>(msg: M) -> Result<T> {
    Err(Error::Caller {
        msg: ErrorMsg::new(msg.as_ref()),
    })
}

pub fn caller_ensure<M: AsRef<str>>(pred: bool, msg: M) -> Result<()> {
    if pred { Ok(()) } else { caller_error(msg) }
}
