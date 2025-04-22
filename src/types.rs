//! Types

pub use ::groundhog_xdr::types::*;
use ::serde::{Serialize, Deserialize};
use ::groundhog_xdr::types as inner;

/*
// Wrap some types ...

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
#[repr(transparent)]
pub struct ContractAddress(
    #[serde(with = "serde_arrays")]
    pub inner::ContractAddress
);

impl From<inner::ContractAddress> for ContractAddress {
    fn from(inner: inner::ContractAddress) -> Self {
        Self(inner)
    }
}

impl AsMut<inner::ContractAddress> for ContractAddress {
    fn as_mut(&mut self) -> &mut inner::ContractAddress {
        &mut self.0
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
#[repr(transparent)]
pub struct StorageKey(
    #[serde(with = "serde_arrays")]
    pub inner::StorageKey
);

impl From<inner::StorageKey> for StorageKey {
    fn from(inner: inner::StorageKey) -> Self {
        Self(inner)
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
#[repr(transparent)]
pub struct Ed25519Signature(
    #[serde(with = "serde_arrays")]
    pub inner::Ed25519Signature
    );

impl From<Ed25519Signature> for Signature {
    fn from(concrete: Ed25519Signature) -> Self {
        Signature::Ed25519 {
            ed25519: concrete.0
        }
    }
}
*/

#[cfg(feature = "mock")]
pub use ::groundhog_xdr::contract::*;
