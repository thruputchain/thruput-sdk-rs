//! Types

pub use xdrs::xdr_common::{VecM, BytesM};

pub use groundhog_xdr::types::*;
pub use groundhog_xdr::syscalls as sys;
pub use groundhog_xdr::syscalls::SysErr;
pub use groundhog_xdr::syscall_error::*;
pub use groundhog_xdr::storage_delta_flags;
pub use groundhog_xdr::contract;
pub use groundhog_xdr::contract::Contract;
pub use groundhog_xdr::contract::ContractEntrypointID;
pub use groundhog_xdr::contract_error;
pub use groundhog_xdr::transaction;
pub use groundhog_xdr::genesis;

// Shorthand for use in contracts
pub use StorageKeyContractData as StorageKey;
