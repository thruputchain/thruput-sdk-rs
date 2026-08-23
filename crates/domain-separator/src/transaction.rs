//! The message a transaction signature is computed over.

use alloc::vec::Vec;

use groundhog_xdr::transaction::Transaction;
use groundhog_xdr::types::NetworkHash;
use xdrs::WriteResult;

use crate::TRANSACTION;

/// The message every transaction signature covers:
///
/// ```text
/// cxs(DomainSeparator("Transaction")) || network_hash || txs(tx)
/// ```
///
/// The domain separator keeps a transaction signature from being reinterpreted
/// as a signature over some other object whose encoding a transaction could be
/// made to imitate. The network hash binds the signature to a single network,
/// so a transaction signed for one chain cannot be replayed on another that
/// happens to carry the same address with the same auth data.
///
/// Note the two encodings: the prefix is CXS (that is how a `DomainSeparator`
/// is encoded everywhere), while the transaction itself is classic XDR — the
/// same bytes as [`xdrs::txs::encode`] alone produced before the preimage
/// gained a prefix. The network hash is a fixed 32-byte opaque, whose CXS and
/// XDR encodings are identical, so it is written verbatim.
///
/// The result is signed directly; ed25519 does its own hashing, so there is no
/// pre-hash here. It must match the C++ validator's
/// `hogd::transaction_signature_preimage` byte for byte, or every signature is
/// rejected.
pub fn transaction_signature_preimage(
    network_hash: &NetworkHash,
    tx: &Transaction,
) -> WriteResult<Vec<u8>> {
    let payload = xdrs::txs::encode(tx)?;

    let prefix = TRANSACTION.as_bytes();
    let mut preimage =
        Vec::with_capacity(prefix.len() + network_hash.len() + payload.len());
    preimage.extend_from_slice(prefix);
    preimage.extend_from_slice(network_hash.as_slice());
    preimage.extend_from_slice(&payload);
    Ok(preimage)
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    use alloc::vec;
    use groundhog_xdr::auth_claim::{
        AuthClaim, AuthClaim_source_address_claim,
    };
    use groundhog_xdr::contract::ContractEntrypointID;
    use groundhog_xdr::transaction::{Invocation, Uniquifier};
    use groundhog_xdr::types::{BlockHeight, ContractAddress, SHA256Hash};
    use xdrs::{BytesM, VecM};

    fn sample_tx() -> Transaction {
        Transaction {
            invocation: Invocation {
                invoked_address: ContractAddress::from([3u8; 32]),
                entrypoint: ContractEntrypointID::default(),
                gas_limit: 100_000,
                calldata: BytesM(vec![1, 2, 3]),
            },
            uniquifier: Uniquifier {
                block_expiry_height: BlockHeight::new(9),
                nonce: 7,
            },
            contracts_to_deploy: VecM(vec![]),
            auth_claim: AuthClaim::kSourceAddressAuthData {
                source_address_claim: AuthClaim_source_address_claim {
                    allowed_keys: 1,
                    required_weight: 1,
                },
            },
        }
    }

    fn network(byte: u8) -> NetworkHash {
        NetworkHash::new(SHA256Hash::new([byte; 32]))
    }

    /// Pins the layout the C++ validator has to reproduce byte for byte.
    #[test]
    fn layout_is_separator_then_network_hash_then_xdr() {
        let tx = sample_tx();
        let hash = network(0x5a);
        let preimage = transaction_signature_preimage(&hash, &tx).unwrap();
        let payload = xdrs::txs::encode(&tx).unwrap();
        let prefix = TRANSACTION.as_bytes();

        assert_eq!(preimage.len(), prefix.len() + 32 + payload.len());
        assert_eq!(&preimage[..prefix.len()], prefix);
        assert_eq!(&preimage[prefix.len()..prefix.len() + 32], &[0x5au8; 32]);
        assert_eq!(&preimage[prefix.len() + 32..], payload.as_slice());
    }

    #[test]
    fn network_hash_changes_the_message() {
        let tx = sample_tx();
        assert_ne!(
            transaction_signature_preimage(&network(0x5a), &tx).unwrap(),
            transaction_signature_preimage(&network(0xa5), &tx).unwrap()
        );
    }
}
