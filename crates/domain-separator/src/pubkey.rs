//! Pubkey account address derivation.

use core::mem::MaybeUninit;

use groundhog_xdr::auth_claim::PubkeyAccountPreimage;
use groundhog_xdr::types::ContractAddress;
use xdrs::cxs::ConstMaxSize;

use crate::PUBKEY_ACCOUNT;

/// Scratch size for [`pubkey_account_address`]: the `"PubkeyAccount"` prefix
/// plus the worst-case CXS size of a [`PubkeyAccountPreimage`].
const SCRATCH: usize = {
    let object_max = match <PubkeyAccountPreimage as ConstMaxSize>::MAX_SIZE {
        Some(n) => n,
        None => panic!("PubkeyAccountPreimage has an unbounded CXS size"),
    };
    PUBKEY_ACCOUNT.as_bytes().len() + object_max
};

/// Derive the on-chain address of a pubkey account from its preimage:
/// `sha256( cxs(DomainSeparator("PubkeyAccount")) || cxs(preimage) )`.
pub fn pubkey_account_address(
    preimage: &PubkeyAccountPreimage,
) -> ContractAddress {
    let mut scratch = [MaybeUninit::<u8>::uninit(); SCRATCH];
    // SAFETY: `SCRATCH` is the prefix length plus `PubkeyAccountPreimage`'s
    // worst-case CXS size (`ConstMaxSize::MAX_SIZE`, checked to be bounded at
    // compile time), so the buffer always holds the encoded preimage.
    // `PubkeyAccountPreimage` has no variable-length fields, so it cannot
    // violate a declared XDR bound.
    let hash = unsafe { PUBKEY_ACCOUNT.hash_into(preimage, &mut scratch) };
    ContractAddress::new(hash.into_inner())
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    use crate::backend::sha256_raw;
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
        let expected = ContractAddress::new(sha256_raw(&manual));
        assert_eq!(pubkey_account_address(&p), expected);
    }

    #[test]
    fn scratch_fits_representative_preimage() {
        // SCRATCH must hold this preimage: with the unchecked encode path a
        // short buffer would be UB, not a panic.
        let _ = pubkey_account_address(&sample());
    }
}
