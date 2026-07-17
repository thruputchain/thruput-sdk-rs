//! Pubkey account address derivation.
//!
//! The derivation is written once, generic over the backend; the public
//! entry points are the per-backend modules below, which bind that generic and
//! so take no type parameter of their own. Naming the backend in the module
//! path is deliberate: it is the one thing a caller must not get wrong by
//! accident, so it is spelled at the import rather than defaulted.

use core::mem::MaybeUninit;

use groundhog_xdr::auth_claim::PubkeyAccountPreimage;
use groundhog_xdr::types::ContractAddress;
use xdrs::cxs::ConstMaxSize;

use crate::PUBKEY_ACCOUNT;
use crate::backend::Sha256Backend;

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
/// with `H`. The backend-bound wrappers in [`crate::sha2`] / [`crate::syscall`]
/// are the public API.
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
    use super::{ContractAddress, PubkeyAccountPreimage, derive};

    /// Derive the on-chain address of a pubkey account from its preimage:
    /// `sha256( cxs(DomainSeparator("PubkeyAccount")) || cxs(preimage) )`.
    pub fn pubkey_account_address(
        preimage: &PubkeyAccountPreimage,
    ) -> ContractAddress {
        derive::<crate::backend::Sha2>(preimage)
    }
}

/// Derivation on the SHA256 syscall — the contract-facing path. Natively this
/// needs a mock dispatch (`syscall-mock`) and a contract frame to serve the
/// syscall; there is no host fallback, by design.
#[cfg(feature = "syscall")]
#[cfg_attr(docsrs, doc(cfg(feature = "syscall")))]
pub mod syscall {
    use super::{ContractAddress, PubkeyAccountPreimage, derive};

    /// Derive the on-chain address of a pubkey account from its preimage:
    /// `sha256( cxs(DomainSeparator("PubkeyAccount")) || cxs(preimage) )`.
    pub fn pubkey_account_address(
        preimage: &PubkeyAccountPreimage,
    ) -> ContractAddress {
        derive::<crate::backend::Syscall>(preimage)
    }
}

#[cfg(all(test, feature = "std", feature = "sha2"))]
mod tests {
    use super::*;
    use crate::backend::Sha2;
    use crate::sha2::pubkey_account_address;
    use groundhog_xdr::types::{Ed25519PublicKey, PublicKey};

    fn sample() -> PubkeyAccountPreimage {
        PubkeyAccountPreimage {
            pubkey: PublicKey::Ed25519 {
                ed25519: Ed25519PublicKey::from([7u8; 32]),
            },
            nonce: 42,
        }
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
