use sdk::{ReadXDR, WriteXDR, SysErr};
use thiserror::Error;

pub type Result<T, E = Error> = core::result::Result<T, E>;

#[derive(Error, Debug, PartialEq, Clone)]
#[derive(ReadXDR, WriteXDR)]
#[non_exhaustive]
#[repr(i32)]
pub enum Error {
    #[error("None")]
    None = 0,

    #[error("Contract already initialized")]
    AlreadyInitialized = 1,

    #[error("Caller must be contract owner")]
    MustBeContractOwner = 2,

    #[error("Internal contract error (system call)")]
    SystemErr = 3,

    #[error("Internal contract error (storage layer)")]
    StorageErr = 4,
}

impl From<SysErr> for Error {
    fn from(_err: SysErr) -> Self {
        Error::SystemErr
    }
}

#[cfg(feature = "storage")]
const _: () = {
    use sdk::storage::error::Error as StorageError;

    impl From<StorageError> for Error {
        fn from(_err: StorageError) -> Self {
            Error::StorageErr
        }
    }
};
