use crate::exn::*;
use crate::sys::{SysErr, SyscallErrorCode, SyscallNo};
use crate::xdr::*;
#[cfg(feature = "debugging")]
use thiserror::Error;

pub type Result<T, E = StorageError> = core::result::Result<T, E>;

#[cfg(feature = "spec")]
use xdrs::{XDR, xdr_repr::XDR};

#[derive(Debug, Clone, PartialEq, ReadXDR, WriteXDR)]
#[cfg_attr(feature = "debugging", derive(Error))]
#[cfg_attr(feature = "spec", derive(XDR))]
#[repr(i32)]
pub enum StorageError {
    #[cfg_attr(feature = "debugging", error(transparent))]
    Sys { exn: SyscallExn },

    #[cfg_attr(
        feature = "debugging",
        error("Storage error due to serialization")
    )]
    Serialization,

    #[cfg_attr(
        feature = "debugging",
        error("Vector index out of bounds: len={len}, access={access}")
    )]
    IndexOutOfBounds { len: u64, access: u64 },
}

impl StorageError {
    pub fn is_storage_key_not_exist(&self) -> bool {
        if let StorageError::Sys { exn } = self {
            exn.has_code(SyscallErrorCode::kSyscallErrorCodeStorageKeyNotExist)
        } else {
            false
        }
    }
}

pub fn oob_throw(len: u64, access: u64) -> ! {
    library_throw(StorageError::IndexOutOfBounds { len, access })
}

pub fn storage_load_throw(_r: ReadError) -> ! {
    library_throw(StorageError::Serialization)
}

pub fn storage_load_error(_r: ReadError) -> StorageError {
    StorageError::Serialization
}

pub fn storage_syserr(e: SysErr) -> StorageError {
    StorageError::Sys {
        exn: SyscallExn::new(SyscallNo::kBytestringGet, e),
    }
}
