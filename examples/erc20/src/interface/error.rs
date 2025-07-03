use sdk::storage_prelude::{Serialize, Deserialize, SysErr};
use sdk::storage::error::Error as StorageError;
use sdk::prelude::{ReadXDR, WriteXDR};
use ::thiserror::Error;

pub type Result<T, E = Error> = core::result::Result<T, E>;

#[derive(Error, Debug, PartialEq, Clone, Serialize, Deserialize, ReadXDR, WriteXDR)]
#[non_exhaustive]
#[repr(i32)]
pub enum Error {
    #[error("None")]
    None,

    #[error("Contract already initialized")]
    AlreadyInitialized,

    #[error("Caller must be contract owner")]
    MustBeContractOwner,

    #[error("Internal contract error (storage layer)")]
    StorageErr,

    #[error("Internal contract error (system call)")]
    SystemErr,
}

impl From<StorageError> for Error {
    fn from(_err: StorageError) -> Self {
        Error::StorageErr
    }
}

impl From<SysErr> for Error {
    fn from(_err: SysErr) -> Self {
        Error::SystemErr
    }
}
