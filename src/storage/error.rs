use crate::serialize::{Deserialize, Serialize};
use crate::syscall::SysErr;
use ::bcs::Error as SerializationError;
use ::thiserror::Error;
use ::xdrs::{ReadXDR, WriteXDR};

pub type Result<T, E = Error> = core::result::Result<T, E>;

#[derive(Error, Debug, Clone, PartialEq, Serialize, Deserialize, ReadXDR, WriteXDR)]
#[repr(i32)]
pub enum Error {
    #[error("Storage error due to {0}")]
    Syscall(#[from] SysErr) = ::groundhog_xdr::contract_error::StorageErrorType::kStorageErrorTypeSyscall as i32,

    #[error("Storage error due to serialization")]
    Serialization = ::groundhog_xdr::contract_error::StorageErrorType::kStorageErrorTypeSerialization as i32,

    #[error("Vector index out of bounds: len={len}, access={access}")]
    IndexOutOfBounds { len: u64, access: u64 } = ::groundhog_xdr::contract_error::StorageErrorType::kStorageErrorTypeIndexOutOfBounds as i32,
}

impl From<SerializationError> for Error {
    fn from(_err: SerializationError) -> Self {
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
