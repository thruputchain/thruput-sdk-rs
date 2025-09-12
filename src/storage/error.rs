use crate::SysErr;
use crate::contract_error::*;
use crate::{ReadXDR, WriteXDR};
use thiserror::Error;

pub type Result<T, E = Error> = core::result::Result<T, E>;

#[cfg(feature = "spec")]
use xdrs::{XDR, xdr_repr::XDR};

#[derive(Error, Debug, Clone, PartialEq, ReadXDR, WriteXDR)]
#[cfg_attr(feature = "spec", derive(XDR))]
#[repr(i32)]
pub enum Error {
    #[error("Storage error due to {syscall_error}")]
    Syscall { #[from] syscall_error: SysErr } = StorageErrorType::kStorageErrorTypeSyscall as i32,

    #[error("Storage error due to serialization")]
    Serialization = StorageErrorType::kStorageErrorTypeSerialization as i32,

    #[error("Vector index out of bounds: len={len}, access={access}")]
    IndexOutOfBounds { len: u64, access: u64 } = StorageErrorType::kStorageErrorTypeIndexOutOfBounds as i32,
}

impl Error {
    pub fn is_storage_key_not_exist(&self) -> bool {
        if let Error::Syscall{syscall_error: err} = self {
            err.is_storage_key_not_exist()
        } else {
            false
        }
    }
}

pub fn oob_err<T>(len: u64, access: u64) -> Result<T> {
    Err(Error::IndexOutOfBounds { len, access })
}
