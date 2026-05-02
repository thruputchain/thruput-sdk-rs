//! Types

pub use xdrs::{BytesM, VecM};

pub use groundhog_xdr::auth_claim::AuthClaim;
pub use groundhog_xdr::auth_claim::AuthClaimType;
pub use groundhog_xdr::auth_claim::KeyID;
pub use groundhog_xdr::contract;
pub use groundhog_xdr::contract::Contract;
pub use groundhog_xdr::contract::ContractEntrypointID;
pub use groundhog_xdr::event;
pub use groundhog_xdr::exec_result::*;
pub use groundhog_xdr::genesis;
pub use groundhog_xdr::storage_cost::*;
pub use groundhog_xdr::storage_item::StorageDeltaFlags;
pub use groundhog_xdr::transaction;
pub use groundhog_xdr::types::*;

pub mod sys {
    pub use groundhog_xdr::SyscallNo;
    pub use groundhog_xdr::syscall_error::SyscallErrorCode;
    pub use groundhog_xdr::syscalls::SysErr;
    pub use groundhog_xdr::syscalls::*;

    pub type SysResult<T = ()> = core::result::Result<T, SysErr>;
}

// Shorthand for use in contracts
pub use StorageKeyContractData as StorageKey;

pub use groundhog_xdr::auth_token::{
    AuthToken, AuthTokenNode, AuthTokenPayload, kAuthTokenMaxChildNodes,
    kAuthTokenMaxDepth, kAuthTokenMaxLen,
};
