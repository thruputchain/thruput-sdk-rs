//! Types

pub use xdrs::{BytesM, VecM};

pub use thruput_xdr::auth_claim::AuthClaim;
pub use thruput_xdr::auth_claim::AuthClaimType;
pub use thruput_xdr::auth_claim::KeyID;
pub use thruput_xdr::auth_claim::PubkeyAccountPreimage;
pub use thruput_xdr::contract;
pub use thruput_xdr::contract::Contract;
pub use thruput_xdr::contract::ContractEntrypointID;
pub use thruput_xdr::event;
pub use thruput_xdr::exec_result::*;
pub use thruput_xdr::genesis;
pub use thruput_xdr::genesis_addresses;
pub use thruput_xdr::retrocausal_queue::kQueueEntryMaxLen;
pub use thruput_xdr::storage_cost::*;
pub use thruput_xdr::storage_item::StorageDeltaFlags;
/// [`StorageDeltaFlags`] under the name storage handles use for it: the flags
/// (today only `ExclusiveLock`) a transaction requests for a key it writes.
pub use thruput_xdr::storage_item::StorageDeltaFlags as StorageFlags;
pub use thruput_xdr::transaction;
pub use thruput_xdr::types::*;

pub mod sys {
    pub use thruput_xdr::SyscallNo;
    pub use thruput_xdr::invoke_args::{
        CopyableInvokeInputs, CopyableInvokeOutputs,
    };
    pub use thruput_xdr::syscall_error::SyscallErrorCode;
    pub use thruput_xdr::syscalls::*;
    pub use thruput_xdr::syserr::SysErr;

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
/// changes it. The leading eight bytes of the hash, kept as bytes: an
/// interface ID is a name anyone may declare, so its width only keeps
/// honestly distinct interfaces apart (as with [`ContractEntrypointID`]),
/// and bytes cross the wire and the interface table with no decoding. This
/// is Thruput's analogue of an ERC-165 interface ID; `thruput-erc165-if`
/// is the query surface for it.
pub type InterfaceId = [u8; 8];

pub use thruput_xdr::auth_token::{
    AuthToken, AuthTokenNode, AuthTokenPayload, kAuthTokenMaxChildNodes,
    kAuthTokenMaxDepth, kAuthTokenMaxLen,
};
