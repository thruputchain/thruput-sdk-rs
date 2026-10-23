//! Routines for reading transaction AuthClaims.
//!
//! These routines are thin wrappers around VM syscalls.
// TODO(geoff): Think about the following questions:
// 1: is there a better interface in which to wrap these calls?
// 2: Do we want some runtime-checked "single-use" checks for these authclaims?

use crate::sys::{self, SysResult};
use core::mem::MaybeUninit;
use thruput_xdr::auth_claim::AuthClaim;

pub use thruput_xdr::auth_claim::BitMap;
pub use thruput_xdr::auth_claim::Weight;
use thruput_xdr::types::PublicKey;

/// The `GetAuthClaim` syscall writes the claim into guest memory in the
/// in-memory layout of the generated `#[repr(i32)]` [`AuthClaim`], which the
/// host mirrors by hand. Pin that layout here so a protocol change that moves
/// it fails to compile instead of handing contracts misread claims.
///
/// A `#[repr(i32)]` enum with fields is laid out as a `#[repr(C)]` union of
/// `#[repr(C)]` structs, one per arm, each starting with the `i32` tag
/// (RFC 2195). Per arm, as offset: size:
///
/// ```text
/// 0: 4   tag (AuthClaimType, i32)            both arms
/// kSourceAddressAuthData:
///   4: 4   allowed_keys (u32)
///   8: 4   required_weight (u32)
///  12: 36  padding (to the union size)
/// kPubkeySignature:
///   4: 4   padding (PubkeyAccountPreimage is 8-aligned)
///   8: 32  pubkey_preimage.pubkey_hash (opaque[32])
///  40: 8   pubkey_preimage.nonce (u64)
/// total 48, align 8
/// ```
///
/// The pubkey arm holds a fixed 32-byte key hash, never the key, so the
/// claim is 48 bytes whatever the signature scheme.
mod layout {
    use core::mem::{align_of, offset_of, size_of};
    use thruput_xdr::auth_claim::{
        AuthClaim, AuthClaim_source_address_claim, AuthClaimType,
        PubkeyAccountPreimage,
    };
    use thruput_xdr::types::PublicKeyHash;

    /// The `kPubkeySignature` arm as the language lays it out. `offset_of!`
    /// cannot yet name an enum variant's fields on stable, so the arm is
    /// spelled out; the runtime test below checks it against a real claim.
    #[repr(C)]
    struct PubkeyArm {
        tag: AuthClaimType,
        pubkey_preimage: PubkeyAccountPreimage,
    }

    /// The `kSourceAddressAuthData` arm, likewise.
    #[repr(C)]
    struct SourceArm {
        tag: AuthClaimType,
        source_address_claim: AuthClaim_source_address_claim,
    }

    pub(super) const AUTH_CLAIM_SIZE: usize = 48;
    pub(super) const AUTH_CLAIM_ALIGN: usize = 8;
    pub(super) const PUBKEY_HASH_OFFSET: usize = 8;
    pub(super) const NONCE_OFFSET: usize = 40;

    const _: () = {
        assert!(size_of::<AuthClaimType>() == 4);
        assert!(size_of::<AuthClaim>() == AUTH_CLAIM_SIZE);
        assert!(align_of::<AuthClaim>() == AUTH_CLAIM_ALIGN);

        assert!(size_of::<PublicKeyHash>() == 32);
        assert!(align_of::<PublicKeyHash>() == 1);
        assert!(size_of::<PubkeyAccountPreimage>() == 40);
        assert!(align_of::<PubkeyAccountPreimage>() == 8);
        assert!(offset_of!(PubkeyAccountPreimage, pubkey_hash) == 0);
        assert!(offset_of!(PubkeyAccountPreimage, nonce) == 32);

        assert!(offset_of!(PubkeyArm, tag) == 0);
        assert!(offset_of!(PubkeyArm, pubkey_preimage) == PUBKEY_HASH_OFFSET);
        assert!(
            PUBKEY_HASH_OFFSET + offset_of!(PubkeyAccountPreimage, nonce)
                == NONCE_OFFSET
        );
        assert!(size_of::<PubkeyArm>() == AUTH_CLAIM_SIZE);

        assert!(size_of::<AuthClaim_source_address_claim>() == 8);
        assert!(align_of::<AuthClaim_source_address_claim>() == 4);
        assert!(offset_of!(AuthClaim_source_address_claim, allowed_keys) == 0);
        assert!(
            offset_of!(AuthClaim_source_address_claim, required_weight) == 4
        );
        assert!(offset_of!(SourceArm, source_address_claim) == 4);
        assert!(size_of::<SourceArm>() == 12);
    };
}

pub fn get_auth_claim() -> SysResult<AuthClaim> {
    let mut auth_claim = MaybeUninit::uninit();
    sys::uninit_GetAuthClaim(&mut auth_claim)?;
    Ok(unsafe { auth_claim.assume_init() })
}

pub fn set_address_required_weight(required_weight: Weight) -> SysResult<()> {
    sys::uninit_WriteAddressRequiredWeight(required_weight)
}

/// Register `public_key` at `pk_index`. ML-DSA-65 keys need a runtime that
/// includes hogd #4539; an older runtime, like any runtime given a scheme it
/// does not know, rejects the key with `InvalidSyscallInput`.
pub fn add_address_public_key(
    pk_index: u32,
    public_key: PublicKey,
    key_weight: Weight,
) -> SysResult<()> {
    let scheme = public_key.scheme();
    sys::uninit_ChangePublicKeyRegistration(
        pk_index,
        Some(&scheme.0),
        crate::crypto::public_key_bytes(&public_key),
        Some(&key_weight),
    )
}

pub fn remove_address_public_key(pk_index: u32) -> SysResult<()> {
    sys::uninit_ChangePublicKeyRegistration(pk_index, None, &[], None)
}

#[cfg(test)]
mod tests {
    use super::layout::*;
    use thruput_xdr::auth_claim::{
        AuthClaim, AuthClaim_source_address_claim, AuthClaimType,
        PubkeyAccountPreimage,
    };
    use thruput_xdr::types::PublicKeyHash;

    /// Byte offset of `field` within `claim`, measured by address so that no
    /// padding byte is ever read.
    fn offset<T>(claim: &AuthClaim, field: &T) -> usize {
        field as *const T as usize - claim as *const AuthClaim as usize
    }

    /// Check the documented offsets against real claims, so the hand-spelled
    /// arms in `layout` cannot drift from what the compiler does. The tag at
    /// offset 0 is what `claim_type` reads.
    #[test]
    fn pubkey_claim_field_offsets() {
        let claim = AuthClaim::kPubkeySignature {
            pubkey_preimage: PubkeyAccountPreimage {
                pubkey_hash: PublicKeyHash::new([1; 32]),
                nonce: 7,
            },
        };
        assert_eq!(claim.claim_type(), AuthClaimType::kPubkeySignature);
        let AuthClaim::kPubkeySignature { pubkey_preimage } = &claim else {
            unreachable!()
        };
        assert_eq!(
            offset(&claim, &pubkey_preimage.pubkey_hash),
            PUBKEY_HASH_OFFSET
        );
        assert_eq!(offset(&claim, &pubkey_preimage.nonce), NONCE_OFFSET);
    }

    #[test]
    fn source_claim_field_offsets() {
        let claim = AuthClaim::kSourceAddressAuthData {
            source_address_claim: AuthClaim_source_address_claim {
                allowed_keys: 1,
                required_weight: 2,
            },
        };
        assert_eq!(claim.claim_type(), AuthClaimType::kSourceAddressAuthData);
        let AuthClaim::kSourceAddressAuthData {
            source_address_claim,
        } = &claim
        else {
            unreachable!()
        };
        assert_eq!(offset(&claim, &source_address_claim.allowed_keys), 4);
        assert_eq!(offset(&claim, &source_address_claim.required_weight), 8);
    }
}
