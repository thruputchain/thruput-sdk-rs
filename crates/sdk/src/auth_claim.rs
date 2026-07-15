//! Routines for reading transaction AuthClaims.
//!
//! These routines are thin wrappers around VM syscalls.
// TODO(geoff): Think about the following questions:
// 1: is there a better interface in which to wrap these calls?
// 2: Do we want some runtime-checked "single-use" checks for these authclaims?

use crate::hash_domain::domain_hash;
use crate::sys::{self, SysResult};
use crate::types::{ContractAddress, SHA256Hash};
use core::mem::MaybeUninit;
use groundhog_xdr::auth_claim::AuthClaim;

pub use groundhog_xdr::auth_claim::BitMap;
pub use groundhog_xdr::auth_claim::Weight;
use groundhog_xdr::syscall_error::SyscallErrorCode;
use groundhog_xdr::types::PublicKey;

pub fn get_auth_claim() -> SysResult<AuthClaim> {
    let mut auth_claim = MaybeUninit::uninit();
    sys::uninit_GetAuthClaim(&mut auth_claim)?;
    Ok(unsafe { auth_claim.assume_init() })
}

pub fn set_address_required_weight(required_weight: Weight) -> SysResult<()> {
    sys::uninit_WriteAddressRequiredWeight(required_weight)
}

fn key_bytes(public_key: &PublicKey) -> SysResult<&[u8]> {
    match public_key {
        PublicKey::Ed25519 { ed25519 } => Ok(ed25519.as_ref()),
        _ => {
            Err(SyscallErrorCode::kSyscallErrorCodeInvalidSyscallInput.into())
        }
    }
}

pub fn add_address_public_key(
    pk_index: u32,
    public_key: PublicKey,
    key_weight: Weight,
) -> SysResult<()> {
    let scheme = public_key.scheme();
    sys::uninit_ChangePublicKeyRegistration(
        pk_index,
        Some(&scheme.0),
        key_bytes(&public_key)?,
        Some(&key_weight),
    )
}

pub fn remove_address_public_key(pk_index: u32) -> SysResult<()> {
    sys::uninit_ChangePublicKeyRegistration(pk_index, None, &[], None)
}

const PUBKEY_ACCOUNT_TAG: SHA256Hash = crate::sha256_static!("PUBKEY_ACCOUNT");

/// Given an [`AuthClaim`] (such as one retrieved from [`get_auth_claim`]),
/// compute the associated `ContractAddress` controlled by the contained public
/// key (or `None` if it is not a public-key auth-claim).
#[must_use]
pub fn auth_claim_pubkey_contract_address(
    auth_claim: &AuthClaim,
) -> Option<ContractAddress> {
    match auth_claim {
        AuthClaim::kPubkeySignature { pubkey_preimage } => {
            let digest =
                domain_hash(PUBKEY_ACCOUNT_TAG.as_ref(), pubkey_preimage);
            let address = digest.into();
            Some(address)
        }
        _ => None,
    }
}
