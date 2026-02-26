use crate::abort;
use crate::exn::*;
use crate::sys::{SysErr, SyscallErrorCode::*, SyscallNo};
use crate::xdr::*;
use thiserror::Error;

pub type Result<T, E = StorageError> = core::result::Result<T, E>;

#[cfg(feature = "spec")]
use xdrs::{XDR, xdr_repr::XDR};

#[derive(Error, Debug, Clone, PartialEq, ReadXDR, WriteXDR)]
#[cfg_attr(feature = "spec", derive(XDR))]
#[repr(i32)]
pub enum StorageError {
    #[error(transparent)]
    Sys { exn: SyscallExn },

    #[error("Storage error due to serialization")]
    Serialization,

    #[error("Vector index out of bounds: len={len}, access={access}")]
    IndexOutOfBounds { len: u64, access: u64 },
}

impl StorageError {
    pub fn is_storage_key_not_exist(&self) -> bool {
        if let StorageError::Sys { exn } = self {
            exn.has_code(kSyscallErrorCodeStorageKeyNotExist)
        } else {
            false
        }
    }
}

pub fn oob_abort(len: u64, access: u64) -> ! {
    abort(&StorageError::IndexOutOfBounds { len, access })
}

pub fn storage_load_abort(_r: ReadError) -> ! {
    library_abort(StorageError::Serialization)
}

pub fn storage_load_error(_r: ReadError) -> StorageError {
    StorageError::Serialization
}

pub fn storage_syserr(e: SysErr) -> StorageError {
    StorageError::Sys {
        exn: SyscallExn::new(SyscallNo::kBytestringGet, e),
    }
}
