use sdk::{ReadXDR, SysErr, WriteXDR};
use thiserror::Error;

pub type Result<T, E = ERC20Error> = core::result::Result<T, E>;

#[cfg(feature = "spec")]
use xdrs::XDR;

#[derive(Error, Debug, PartialEq, Clone, ReadXDR, WriteXDR)]
#[cfg_attr(feature = "spec", derive(XDR))]
#[non_exhaustive]
#[repr(i32)]
pub enum ERC20Error {
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

impl From<SysErr> for ERC20Error {
    fn from(_err: SysErr) -> Self {
        ERC20Error::SystemErr
    }
}

#[cfg(feature = "storage")]
const _: () = {
    use sdk::storage::error::Error as StorageError;

    impl From<StorageError> for ERC20Error {
        fn from(_err: StorageError) -> Self {
            ERC20Error::StorageErr
        }
    }
};

// Contract routines for generating errors

pub fn internal_err<T, M: AsRef<str>>(_msg: M) -> Result<T> {
    Err(ERC20Error::SystemErr)
}

pub fn caller_error<T>(err: ERC20Error) -> Result<T> {
    Err(err)
}

pub fn caller_ensure(pred: bool, err: ERC20Error) -> Result<()> {
    if pred { Ok(()) } else { Err(err) }
}

pub fn amount_to_diff(n: u128) -> Result<i128> {
    n.try_into().map_err(|_| ERC20Error::TooLarge)
}
