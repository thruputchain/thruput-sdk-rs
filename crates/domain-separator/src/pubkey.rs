//! Pubkey account address derivation, and the public key hash it names the
//! account's key by.
//!
//! An account's address is
//! `sha256( cxs(DomainSeparator("PubkeyAccount")) || cxs(PubkeyAccountPreimage) )`,
//! and the preimage holds the key's [`PublicKeyHash`],
//! `sha256( cxs(DomainSeparator("PublicKey")) || cxs(PublicKey) )`, rather than
//! the key. The hash covers the scheme, so an Ed25519 key and an ML-DSA-65 key
//! never share an address. A contract re-derives its own address from the
//! auth claim alone ([`PubkeyAccountPreimage`] is all it needs); only a client
//! building the claim needs the key, to hash it.
//!
//! The derivation is written once, generic over the backend; the public
//! entry points are the per-backend modules below, which bind that generic and
//! so take no type parameter of their own. Naming the backend in the module
//! path is deliberate: it is the one thing a caller must not get wrong by
//! accident, so it is spelled at the import rather than defaulted.

use core::mem::MaybeUninit;

use thruput_xdr::auth_claim::PubkeyAccountPreimage;
use thruput_xdr::types::{
    ContractAddress, Ed25519PublicKey, MlDsa65PublicKey, PublicKey,
    PublicKeyHash,
};
use xdrs::cxs::ConstMaxSize;

use crate::backend::Sha256Backend;
use crate::{PUBKEY_ACCOUNT, PUBLIC_KEY};

/// Scratch size for [`key_hash`]: the `"PublicKey"` prefix plus the
/// worst-case CXS size of a [`PublicKey`] (an ML-DSA-65 key).
const KEY_SCRATCH: usize = {
    // Boxed enum arms deliberately have no recursive ConstMaxSize bound.
    // Size the known key payloads and discriminant, then use checked encoding.
    let ed25519 = <Ed25519PublicKey as ConstMaxSize>::MAX_SIZE.unwrap();
    let mldsa65 = <MlDsa65PublicKey as ConstMaxSize>::MAX_SIZE.unwrap();
    let payload = if ed25519 > mldsa65 { ed25519 } else { mldsa65 };
    let object_max = <u32 as ConstMaxSize>::MAX_SIZE.unwrap() + payload;
    PUBLIC_KEY.as_bytes().len() + object_max
};

/// `sha256( cxs(DomainSeparator("PublicKey")) || cxs(public_key) )`, hashed
/// with `H`.
fn key_hash<H: Sha256Backend>(public_key: &PublicKey) -> PublicKeyHash {
    let mut scratch = [MaybeUninit::<u8>::uninit(); KEY_SCRATCH];
    let prefix = PUBLIC_KEY.as_bytes();
    for (slot, byte) in scratch.iter_mut().zip(prefix) {
        slot.write(*byte);
    }
    let size =
        xdrs::cxs::encode_into(public_key, &mut scratch[prefix.len()..])
            .expect("public key exceeds the supported CXS size");
    // SAFETY: the prefix and the successful checked encoder initialized
    // exactly this slice. A future larger key fails before hashing.
    let bytes = unsafe {
        core::slice::from_raw_parts(
            scratch.as_ptr().cast::<u8>(),
            prefix.len() + size,
        )
    };
    PublicKeyHash::new(H::sha256(bytes))
}

/// Scratch size for [`derive`]: the `"PubkeyAccount"` prefix plus the
/// worst-case CXS size of a [`PubkeyAccountPreimage`].
const SCRATCH: usize = {
    let object_max = match <PubkeyAccountPreimage as ConstMaxSize>::MAX_SIZE {
        Some(n) => n,
        None => panic!("PubkeyAccountPreimage has an unbounded CXS size"),
    };
    PUBKEY_ACCOUNT.as_bytes().len() + object_max
};

/// `sha256( cxs(DomainSeparator("PubkeyAccount")) || cxs(preimage) )`, hashed
/// with `H`. The backend-bound wrappers in the `sha2` / `syscall` modules are
/// the public API.
fn derive<H: Sha256Backend>(
    preimage: &PubkeyAccountPreimage,
) -> ContractAddress {
    let mut scratch = [MaybeUninit::<u8>::uninit(); SCRATCH];
    // SAFETY: `SCRATCH` is the prefix length plus `PubkeyAccountPreimage`'s
    // worst-case CXS size (`ConstMaxSize::MAX_SIZE`, checked to be bounded at
    // compile time), so the buffer always holds the encoded preimage.
    // `PubkeyAccountPreimage` has no variable-length fields, so it cannot
    // violate a declared XDR bound.
    let hash =
        unsafe { PUBKEY_ACCOUNT.hash_into::<H, _>(preimage, &mut scratch) };
    ContractAddress::new(hash.into_inner())
}

/// Derivation on the in-process `sha2` backend — for host tooling deriving an
/// address outside any contract frame.
#[cfg(feature = "sha2")]
#[cfg_attr(docsrs, doc(cfg(feature = "sha2")))]
pub mod sha2 {
    use super::{
        ContractAddress, PubkeyAccountPreimage, PublicKey, PublicKeyHash,
        derive, key_hash,
    };

    /// Derive the on-chain address of a pubkey account from its preimage:
    /// `sha256( cxs(DomainSeparator("PubkeyAccount")) || cxs(preimage) )`.
    pub fn pubkey_account_address(
        preimage: &PubkeyAccountPreimage,
    ) -> ContractAddress {
        derive::<crate::backend::Sha2>(preimage)
    }

    /// The hash a [`PubkeyAccountPreimage`] names its key by:
    /// `sha256( cxs(DomainSeparator("PublicKey")) || cxs(public_key) )`.
    pub fn public_key_hash(public_key: &PublicKey) -> PublicKeyHash {
        key_hash::<crate::backend::Sha2>(public_key)
    }
}

/// Derivation on the SHA256 syscall — the contract-facing path. Natively this
/// needs a mock dispatch (`syscall-mock`) and a contract frame to serve the
/// syscall; there is no host fallback, by design.
#[cfg(feature = "syscall")]
#[cfg_attr(docsrs, doc(cfg(feature = "syscall")))]
pub mod syscall {
    use super::{
        ContractAddress, PubkeyAccountPreimage, PublicKey, PublicKeyHash,
        derive, key_hash,
    };

    /// Derive the on-chain address of a pubkey account from its preimage:
    /// `sha256( cxs(DomainSeparator("PubkeyAccount")) || cxs(preimage) )`.
    pub fn pubkey_account_address(
        preimage: &PubkeyAccountPreimage,
    ) -> ContractAddress {
        derive::<crate::backend::Syscall>(preimage)
    }

    /// The hash a [`PubkeyAccountPreimage`] names its key by:
    /// `sha256( cxs(DomainSeparator("PublicKey")) || cxs(public_key) )`.
    pub fn public_key_hash(public_key: &PublicKey) -> PublicKeyHash {
        key_hash::<crate::backend::Syscall>(public_key)
    }
}

#[cfg(all(test, feature = "std", feature = "sha2"))]
mod tests {
    use super::*;
    use crate::backend::Sha2;
    use crate::sha2::{pubkey_account_address, public_key_hash};

    fn ed25519(fill: u8) -> PublicKey {
        PublicKey::Ed25519 {
            ed25519: Ed25519PublicKey::from([fill; 32]),
        }
    }

    fn mldsa65(fill: u8) -> PublicKey {
        PublicKey::MlDsa65 {
            mldsa65: MlDsa65PublicKey::from([fill; 1952]).into(),
        }
    }

    fn sample() -> PubkeyAccountPreimage {
        PubkeyAccountPreimage {
            pubkey_hash: public_key_hash(&ed25519(7)),
            nonce: 42,
        }
    }

    #[test]
    fn key_hash_matches_manual_derivation() {
        for key in [ed25519(7), mldsa65(7)] {
            let mut manual = PUBLIC_KEY.as_bytes().to_vec();
            manual.extend_from_slice(&xdrs::cxs::encode(&key).unwrap());
            let expected = PublicKeyHash::new(Sha2::sha256(&manual));
            assert_eq!(public_key_hash(&key), expected);
        }
    }

    /// The hash covers the scheme: the same leading bytes under another
    /// scheme hash differently, and the CXS encoding carries the
    /// discriminant (1 byte) ahead of the key bytes.
    #[test]
    fn key_hash_covers_scheme() {
        assert_ne!(public_key_hash(&ed25519(7)), public_key_hash(&mldsa65(7)));
        assert_eq!(xdrs::cxs::encode(&ed25519(7)).unwrap().len(), 1 + 32);
        assert_eq!(xdrs::cxs::encode(&mldsa65(7)).unwrap().len(), 1 + 1952);
    }

    #[test]
    fn key_scratch_fits_largest_key() {
        // The checked encoding must fit even the largest supported key.
        assert!(KEY_SCRATCH >= PUBLIC_KEY.as_bytes().len() + 1 + 1952);
        let _ = public_key_hash(&mldsa65(0xff));
    }

    #[test]
    fn matches_manual_derivation() {
        let p = sample();
        let mut manual = PUBKEY_ACCOUNT.as_bytes().to_vec();
        manual.extend_from_slice(&xdrs::cxs::encode(&p).unwrap());
        let expected = ContractAddress::new(Sha2::sha256(&manual));
        assert_eq!(pubkey_account_address(&p), expected);
    }

    #[test]
    fn scratch_fits_representative_preimage() {
        // SCRATCH must hold this preimage: with the unchecked encode path a
        // short buffer would be UB, not a panic.
        let _ = pubkey_account_address(&sample());
    }
}
