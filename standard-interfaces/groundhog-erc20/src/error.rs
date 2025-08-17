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

    #[error("Provided amount too large")]
    TooLarge = 3,

    #[error("Internal contract error (system call)")]
    SystemErr = 4,

    #[error("Internal contract error (storage layer)")]
    StorageErr = 5,
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

// Contract routines for generating errors

pub fn internal_err<T, M: AsRef<str>>(_msg: M) -> Result<T> {
    Err(Error::SystemErr)
}

pub fn caller_error<T>(err: Error) -> Result<T> {
    Err(err)
}

pub fn caller_ensure(pred: bool, err: Error) -> Result<()> {
    if pred {
        Ok(())
    } else {
        Err(err)
    }
}

pub fn amount_to_diff(n: u128) -> Result<i128> {
    n.try_into().map_err(|_| Error::TooLarge)
}
