use crate::syscall::SysErr;
use thiserror::Error;
use xdrs::{ReadXDR, WriteXDR, ReadError, WriteError};

pub type Result<T, E = Error> = core::result::Result<T, E>;

#[derive(Error, Debug, Clone, PartialEq, ReadXDR, WriteXDR)]
pub enum Error {
    #[error("resource error due to {0}")]
    Syscall(#[from] SysErr),

    #[error("resource error due to serialization")]
    Serialization,

    #[error("vector index out of bounds: len={len}, access={access}")]
    IndexOutOfBounds { len: u64, access: u64 },
}

impl From<ReadError> for Error {
    fn from(_err: ReadError) -> Self {
        Error::Serialization
    }
}

impl From<WriteError> for Error {
    fn from(_err: WriteError) -> Self {
        Error::Serialization
    }
}

impl Error {
    pub fn is_storage_key_not_exist(&self) -> bool {
        if let Error::Syscall(err) = self {
            err.is_storage_key_not_exist()
        } else {
            false
        }
    }
}

pub fn oob_err<T>(len: u64, access: u64) -> Result<T> {
    Err(Error::IndexOutOfBounds { len, access })
}
