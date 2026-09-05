//! Types

pub use xdrs::{BytesM, VecM};

pub use groundhog_xdr::auth_claim::AuthClaim;
pub use groundhog_xdr::auth_claim::AuthClaimType;
pub use groundhog_xdr::auth_claim::KeyID;
pub use groundhog_xdr::auth_claim::PubkeyAccountPreimage;
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
    pub use groundhog_xdr::invoke_args::{
        CopyableInvokeInputs, CopyableInvokeOutputs,
    };
    pub use groundhog_xdr::syscall_error::SyscallErrorCode;
    pub use groundhog_xdr::syscalls::*;
    pub use groundhog_xdr::syserr::SysErr;

    pub type SysResult<T = ()> = core::result::Result<T, SysErr>;
}

// Shorthand for use in contracts
pub use StorageKeyContractData as StorageKey;

/// The identity of a contract interface at a major version.
///
/// `#[contract]` on a trait derives it from the interface's effective name
/// and major version — the same pair that namespaces the interface's
/// entrypoint IDs — and exposes it as `Trait::INTERFACE_ID` and
/// `<Iface>Entrypoints::INTERFACE_ID`. Two contracts implementing the same
/// interface crate therefore agree on the ID, and a major-version bump
/// changes it. This is Groundhog's analogue of an ERC-165 interface ID;
/// `groundhog-erc165-if` is the query surface for it.
pub type InterfaceId = SHA256Hash;

pub use groundhog_xdr::auth_token::{
    AuthToken, AuthTokenNode, AuthTokenPayload, kAuthTokenMaxChildNodes,
    kAuthTokenMaxDepth, kAuthTokenMaxLen,
};
