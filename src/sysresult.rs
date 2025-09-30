use core::fmt;
use thiserror::Error;
use xdrs::{ReadXDR, WriteXDR};
use ::groundhog_xdr::syscalls::SysErr;

#[cfg(feature = "mock")]
use alloc::string::{String, ToString};

// For whose only error is a SysErr (syscalls or syscall-wrappers)
pub type SysResult<T> = core::result::Result<T, SysErr>;

// For other, more general SDK functions
pub type Result<T> = core::result::Result<T, Error>;

// Error type for general SDK functions
#[derive(Error, Debug, Clone, ReadXDR, WriteXDR)]
#[repr(i32)]
pub enum Error {
    #[error("System call error: {0}")]
    SystemErr(#[from] SysErr),

    #[error("Serialization error{0}")]
    SerializationErr(ErrorMsg),
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
pub struct ErrorMsg(String);

impl ErrorMsg {
    #[cfg(feature = "mock")]
    pub fn new<M: fmt::Display>(msg: M) -> Self {
        Self(msg.to_string())
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
        Self(msg)
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
