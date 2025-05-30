use sdk::storage_prelude::{Serialize, Deserialize, SysErr};
use sdk::storage::error::Error as StorageError;
use ::thiserror::Error;

pub type Result<T, E = Error> = core::result::Result<T, E>;

#[derive(Error, Debug, PartialEq, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Error {
    #[error("Contract already initialized")]
    AlreadyInitialized,

    #[error("Caller must be contract owner")]
    MustBeContractOwner,

    #[error("Internal contract error: {0:?}")]
    ContractErr(#[from] ContractError),
}

#[derive(Error, Debug, PartialEq, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ContractError {
    #[error("Storage-layer error")]
    StorageErr,

    #[error("System error")]
    SystemErr,
}

impl From<StorageError> for ContractError {
    fn from(_err: StorageError) -> Self {
        ContractError::StorageErr
    }
}

impl From<StorageError> for Error {
    fn from(err: StorageError) -> Self {
        ContractError::from(err).into()
    }
}

impl From<SysErr> for ContractError {
    fn from(_err: SysErr) -> Self {
        ContractError::SystemErr
    }
}

impl From<SysErr> for Error {
    fn from(err: SysErr) -> Self {
        ContractError::from(err).into()
    }
}
