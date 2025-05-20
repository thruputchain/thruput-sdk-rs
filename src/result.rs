use crate::syscall::SysErr;
use bcs::Error as SerializationError;
use core::fmt;
use serde::{Deserialize, Serialize};
use thiserror::Error;

// For whose only error is a SysErr (syscalls or syscall-wrappers)
pub type SysResult<T> = core::result::Result<T, SysErr>;

// For other, more general SDK functions
pub type Result<T> = core::result::Result<T, Error>;

// Error type for general SDK functions
#[derive(Error, Debug, Clone, Serialize, Deserialize)]
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
}

#[cfg(feature = "mock")]
use alloc::borrow::ToOwned;

#[cfg(feature = "mock")]
use alloc::string::String;

#[cfg(feature = "mock")]
use alloc::string::ToString;

#[cfg(feature = "mock")]
impl From<SerializationError> for Error {
    fn from(err: SerializationError) -> Self {
        Self::SerializationErr(err.to_string().into())
    }
}

#[cfg(not(feature = "mock"))]
impl From<SerializationError> for Error {
    fn from(_err: SerializationError) -> Self {
        Self::SerializationErr(ErrorMsg::empty())
    }
}

// Error strings will be preserved for testing, but
// dropped when compiled for contract deployment

#[cfg(not(feature = "mock"))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorMsg;

#[cfg(feature = "mock")]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorMsg(String);

impl ErrorMsg {
    #[cfg(feature = "mock")]
    pub fn new(msg: &str) -> Self {
        Self(msg.to_owned())
    }

    #[cfg(not(feature = "mock"))]
    pub fn new(_msg: &str) -> Self {
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
