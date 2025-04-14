use crate::syscall::SysErr;

use alloc::string::{String, ToString};
use alloc::{vec, vec::Vec};
use bcs::Error as SerializationError;
use serde::{Deserialize, Serialize};
use thiserror::Error;

// For whose only error is a SysErr (syscalls or syscall-wrappers)
pub type SysResult<T> = core::result::Result<T, SysErr>;

// For other, more general SDK functions
pub type Result<T> = core::result::Result<T, Error>;

// TODO(geoff): Do we want a configure to toggle detailed error strings?

// Error type for general SDK functions
#[derive(Error, Debug, Serialize, Deserialize)]
pub enum Error {
    #[error("{context:?}: System call error: {err}")]
    SystemErr { err: SysErr, context: Vec<String> },

    #[error("Vector index out of bounds: {0}")]
    IndexOutOfBounds(u64),

    #[error("IO error: {0}")]
    IO(String),

    #[error("Initialization guard failed")]
    InitGuardFailure,

    #[error("Serialization error: {0}")]
    SerializationErr(String),

    #[error("Assertion failed: {0}")]
    AssertionFailed(String),

    #[error("Application error: {0}")]
    AppErr(String),
}

impl Error {
    pub fn get_sys_err(&self) -> Option<SysErr> {
        if let Error::SystemErr { err, .. } = self {
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

impl From<SysErr> for Error {
    fn from(err: SysErr) -> Self {
        Self::SystemErr {
            err,
            context: vec![],
        }
    }
}

impl From<SerializationError> for Error {
    fn from(err: SerializationError) -> Self {
        Self::SerializationErr(err.to_string())
    }
}

pub fn with_context<S: AsRef<str>>(msg: S) -> impl Fn(Error) -> Error {
    move |source| match source {
        Error::SystemErr { err, mut context } => {
            context.push(msg.as_ref().to_string());
            Error::SystemErr { err, context }
        }
        _ => source,
    }
}

pub fn ensure<S: AsRef<str>>(condition: bool, msg: S) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(Error::AssertionFailed(msg.as_ref().to_string()))
    }
}

pub fn check_err<F, T>(thunk: F) -> T
where
    F: Fn() -> Result<T>,
{
    match thunk() {
        Ok(x) => x,
        Err(e) => {
            panic!("{}", e);
        }
    }
}

pub fn oob_err<T>(index: u64) -> Result<T> {
    Err(Error::IndexOutOfBounds(index))
}

pub fn app_err<T>(msg: String) -> Result<T> {
    Err(Error::AppErr(msg))
}
