use crate::InvokeError;
use crate::SysErr;
use crate::serialize::{ReadXDR, WriteError, WriteXDR};
use crate::abort;
use core::fmt;
use thiserror::Error;

#[cfg(feature = "mock")]
use alloc::string::{String, ToString};

// For whose only error is a SysErr (syscalls or syscall-wrappers)
pub type SysResult<T> = core::result::Result<T, SysErr>;

// For other, more general SDK functions
pub type Result<T> = core::result::Result<T, Error>;

// Error type for general SDK functions
#[derive(Error, Debug, Clone, PartialEq, ReadXDR, WriteXDR)]
#[repr(i32)]
pub enum Error {
    #[error("System call error: {0}")]
    SystemErr(#[from] SysErr),

    #[error("Invoke error: {0}")]
    InvokeErr(#[from] InvokeError),

    #[error("Serialization error{0}")]
    SerializationErr(ErrorMsg),

    #[error("Non-negative integer overflow")]
    NonNegativeIntOverflow,

    #[error("Non-negative integer underflow")]
    NonNegativeIntUnderflow,

    #[error("Integer overflow")]
    IntegerOverflow,
}

impl Error {
    pub fn get_sys_err(&self) -> Option<SysErr> {
        if let Error::SystemErr(err) = self {
            Some(*err)
        } else {
            None
        }
    }

    pub fn is_storage_key_not_exist(&self) -> bool {
        self.get_sys_err()
            .map(SysErr::is_storage_key_not_exist)
            .unwrap_or(false)
    }

    pub fn serialization<M: fmt::Display>(msg: M) -> Self {
        Error::SerializationErr(ErrorMsg::new(msg))
    }
}

pub fn nnint_overflow() -> ! {
    abort(&Error::NonNegativeIntOverflow);
}

pub fn nnint_underflow() -> Error {
    Error::NonNegativeIntUnderflow
}

impl From<WriteError> for Error {
    fn from(e: WriteError) -> Self {
        Error::serialization(e)
    }
}

// Error strings will be preserved for testing, but
// dropped when compiled for contract deployment

#[cfg(feature = "spec")]
use xdrs::XDR;

#[cfg(not(feature = "mock"))]
#[derive(Debug, Clone, PartialEq, ReadXDR, WriteXDR)]
#[cfg_attr(feature = "spec", derive(XDR))]
pub struct ErrorMsg;

#[cfg(feature = "mock")]
#[derive(Debug, Clone, PartialEq, ReadXDR, WriteXDR)]
#[cfg_attr(feature = "spec", derive(XDR))]
pub struct ErrorMsg(xdrs::StringM);

impl ErrorMsg {
    #[cfg(feature = "mock")]
    pub fn new<M: fmt::Display>(msg: M) -> Self {
        Self(msg.to_string().parse().unwrap())
    }

    #[cfg(not(feature = "mock"))]
    pub fn new<M: fmt::Display>(_msg: M) -> Self {
        Self
    }

    #[cfg(not(feature = "mock"))]
    pub fn empty() -> Self {
        Self
    }
}

#[cfg(feature = "mock")]
impl From<String> for ErrorMsg {
    fn from(msg: String) -> Self {
        Self(msg.parse().unwrap())
    }
}

#[cfg(feature = "mock")]
impl fmt::Display for ErrorMsg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, ": {}", self.0)
    }
}

#[cfg(not(feature = "mock"))]
impl fmt::Display for ErrorMsg {
    fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
        Ok(())
    }
}
