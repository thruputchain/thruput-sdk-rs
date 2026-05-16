use crate::exn::*;
use crate::sys::{SysErr, SyscallErrorCode, SyscallNo};
use crate::xdr::*;
#[cfg(feature = "debugging")]
use thiserror::Error;

#[cfg(feature = "spec")]
use xdrs::{XDR, xdr_repr::XDR};
use crate::ContractTypeSpec;

#[derive(Debug, Clone, PartialEq, ReadXDR, WriteXDR)]
#[cfg_attr(feature = "debugging", derive(Error))]
#[cfg_attr(feature = "spec", derive(XDR))]
#[derive(ContractTypeSpec)]
#[repr(i32)]
pub enum StorageError {
    #[cfg_attr(feature = "debugging", error(transparent))]
    Sys { exn: SyscallExn } = 0,

    #[cfg_attr(
        feature = "debugging",
        error("Storage error due to serialization")
    )]
    Serialization = 1,

    #[cfg_attr(
        feature = "debugging",
        error("Vector index out of bounds: len={len}, access={access}")
    )]
    IndexOutOfBounds { len: u64, access: u64 } = 2,
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

pub fn throw_load_exn(_r: ReadError) -> ! {
    library_throw(StorageError::Serialization)
}

pub fn throw_store_exn(_: WriteError) -> ! {
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

impl crate::__contract_spec::ContractTypeName for StorageError {
    const NAME: &'static str = "StorageError";
}
