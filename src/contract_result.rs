use crate::result::ErrorMsg;
use crate::serialize::{Deserialize, Serialize};
#[cfg(feature = "storage")]
use crate::storage::error::Error as StorageErr;
use crate::syscall::SysErr;
use ::thiserror::Error;
use ::xdrs::{ReadXDR, WriteXDR};

#[derive(Error, Debug, Clone, PartialEq, Serialize, Deserialize, ReadXDR, WriteXDR)]
#[repr(i32)]
pub enum Error {
    #[error("Caller error{0}")]
    Caller(ErrorMsg) = ::groundhog_xdr::contract_error::ErrorType::kErrorTypeCaller as i32,

    #[error("Contract internal error{0}")]
    Contract(#[from] ContractError) = ::groundhog_xdr::contract_error::ErrorType::kErrorTypeContract as i32,
}

#[derive(Error, Debug, Clone, PartialEq, Serialize, Deserialize, ReadXDR, WriteXDR)]
#[repr(i32)]
pub enum ContractError {
    #[error(": {0}")]
    Custom(ErrorMsg) = ::groundhog_xdr::contract_error::ContractErrorType::kContractErrorTypeCustom as i32,

    #[error(": failed due to syscall error: {0}")]
    Syscall(#[from] SysErr) = ::groundhog_xdr::contract_error::ContractErrorType::kContractErrorTypeSyscall as i32,

    #[cfg(feature = "storage")]
    #[error(": failed due to storage error: {0}")]
    Storage(#[from] StorageErr) = ::groundhog_xdr::contract_error::ContractErrorType::kContractErrorTypeStorage as i32,
}

impl From<SysErr> for Error {
    fn from(err: SysErr) -> Self {
        Error::Contract(err.into())
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

pub type Result<T, E = Error> = core::result::Result<T, E>;

pub fn internal_err<T, M: AsRef<str>>(msg: M) -> Result<T> {
    let msg = ErrorMsg::new(msg.as_ref());
    let contract_err = ContractError::Custom(msg);
    let error = Error::Contract(contract_err);

    Err(error)
}

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
