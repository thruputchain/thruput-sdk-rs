use crate::SysErr;
use crate::{ReadXDR, WriteXDR};
use thiserror::Error;

pub type Result<T, E = StorageError> = core::result::Result<T, E>;

#[cfg(feature = "spec")]
use xdrs::{XDR, xdr_repr::XDR};

#[derive(Error, Debug, Clone, PartialEq, ReadXDR, WriteXDR)]
#[cfg_attr(feature = "spec", derive(XDR))]
#[repr(i32)]
pub enum StorageError {
    #[error("Storage error due to {syscall_error}")]
    Syscall {
        #[from]
        syscall_error: SysErr,
    },

    #[error("Storage error due to serialization")]
    Serialization,

    #[error("Vector index out of bounds: len={len}, access={access}")]
    IndexOutOfBounds { len: u64, access: u64 },
}

impl StorageError {
    pub fn is_storage_key_not_exist(&self) -> bool {
        if let StorageError::Syscall { syscall_error: err } = self {
            err.is_storage_key_not_exist()
        } else {
            false
        }
    }
}

pub fn oob_err<T>(len: u64, access: u64) -> Result<T> {
    Err(StorageError::IndexOutOfBounds { len, access })
}
