//! The message a signed-operation signature is computed over.
//!
//! A transaction signature covers a whole transaction and is checked by the
//! validator. An *operation* signature covers a structured application message
//! and is checked by a contract, so the signer need not be the submitter: the
//! pattern behind permits, session keys and delegated wallet actions. Nothing
//! here reaches the node.

use alloc::vec::Vec;

use groundhog_contract_spec::ContractTypeSpec;
use groundhog_xdr::transaction::Uniquifier;
use groundhog_xdr::types::{ContractAddress, NetworkHash};
use xdrs::{WriteResult, WriteXDR, Writer, WriterUnchecked};

use crate::SIGNED_OP;

/// A structured message authorizing an on-chain action, signed off-chain.
///
/// The signature covers [`preimage`](Self::preimage):
///
/// ```text
/// cxs(DomainSeparator("SignedOp")) || network || verifying_contract
///     || cxs(T::TYPE_DEF) || cxs(uniquifier) || cxs(body)
/// ```
///
/// where `cxs(uniquifier)` is `cxs(block_expiry_height) || cxs(nonce)`, the
/// chain's own [`Uniquifier`] encoding. Each field closes one replay path:
/// `network` stops replay on another network carrying the same address,
/// `verifying_contract` stops replay against another contract, the
/// uniquifier stops replay against the same one — consumed once, dead after
/// its expiry, as the transaction replay cache treats it — and `T::TYPE_DEF`
/// stops the signature being read as a different kind of message. That last one matters because CXS concatenates
/// struct fields with no framing: `Grant { amount: u64 }` and
/// `Revoke { id: u64 }` encode identically, and a `Signed<_>` auth-token field
/// encodes to nothing at all. `#[contract_type]` already emits the type's
/// definition as a const, so it goes into the preimage as-is: adding or
/// renaming a field of `T` retires every old signature.
///
/// The result is signed directly, like `transaction_signature_preimage`;
/// ed25519 does its own hashing.
#[derive(Debug)]
pub struct SignedOp<'a, T: ?Sized> {
    /// The network this operation is valid on.
    pub network: NetworkHash,
    /// The contract entitled to act on this operation.
    pub verifying_contract: ContractAddress,
    /// The signer-chosen nonce and the last height (inclusive) at which the
    /// operation may execute; consumed as a pair by the verifying contract.
    pub uniquifier: Uniquifier,
    /// The application payload, a `#[contract_type]`.
    pub body: &'a T,
}

impl<T: WriteXDR + ContractTypeSpec + ?Sized> SignedOp<'_, T> {
    /// The bytes a signature covers: `cxs(SIGNED_OP) || cxs(self)`.
    pub fn preimage(&self) -> WriteResult<Vec<u8>> {
        SIGNED_OP.preimage(self)
    }
}

// Hand-written for the same reason as `AuthTokenPreimage` in the SDK: the
// derive does not synthesize bounds for `T`.
impl<T: WriteXDR + ContractTypeSpec + ?Sized> WriteXDR for SignedOp<'_, T> {
    fn write_xdr<F, W: Writer<F>>(&self, w: &mut W) -> WriteResult<()> {
        self.network.write_xdr(w)?;
        self.verifying_contract.write_xdr(w)?;
        T::TYPE_DEF.write_xdr(w)?;
        self.uniquifier.write_xdr(w)?;
        self.body.write_xdr(w)
    }

    unsafe fn write_xdr_unchecked<F, W: WriterUnchecked<F>>(&self, w: &mut W) {
        // SAFETY: forwarded caller contract.
        unsafe {
            self.network.write_xdr_unchecked(w);
            self.verifying_contract.write_xdr_unchecked(w);
            T::TYPE_DEF.write_xdr_unchecked(w);
            self.uniquifier.write_xdr_unchecked(w);
            self.body.write_xdr_unchecked(w);
        }
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    use groundhog_contract_spec::ContractTypeName;
    use groundhog_macros::ContractTypeSpec as DeriveContractTypeSpec;
    use groundhog_xdr::types::{BlockHeight, SHA256Hash};

    // Fixtures derive their spec impls through the same derive
    // `#[contract_type]` runs, so they cannot claim a shape the real macro
    // would never emit.

    #[derive(WriteXDR, DeriveContractTypeSpec)]
    #[contract_type_spec(crate = "crate::__sdk_paths")]
    struct Grant {
        amount: u64,
    }
    impl ContractTypeName for Grant {
        const NAME: &'static str = "Grant";
    }

    /// Byte-identical to `Grant` as a value; only `TYPE_DEF` separates them.
    #[derive(WriteXDR, DeriveContractTypeSpec)]
    #[contract_type_spec(crate = "crate::__sdk_paths")]
    struct Revoke {
        id: u64,
    }
    impl ContractTypeName for Revoke {
        const NAME: &'static str = "Revoke";
    }

    /// `Grant` with a field added and the name kept: what a name-based
    /// binding would silently keep accepting.
    #[derive(WriteXDR, DeriveContractTypeSpec)]
    #[contract_type_spec(crate = "crate::__sdk_paths")]
    struct GrantV2 {
        amount: u64,
        recipient: u64,
    }
    impl ContractTypeName for GrantV2 {
        const NAME: &'static str = "Grant";
    }

    fn op<T>(body: &T) -> SignedOp<'_, T> {
        SignedOp {
            network: NetworkHash::new(SHA256Hash::new([0x5a; 32])),
            verifying_contract: ContractAddress::from([0x03; 32]),
            uniquifier: Uniquifier {
                block_expiry_height: BlockHeight::new(1_000),
                nonce: 1,
            },
            body,
        }
    }

    /// Pins the layout the TypeScript client has to reproduce byte for byte.
    /// The uniquifier's two fields land expiry first, then nonce: the order
    /// `Uniquifier` declares them, so a client can encode the chain type.
    #[test]
    fn layout_is_separator_header_type_def_then_body() {
        let body = Grant { amount: 42 };
        let preimage = op(&body).preimage().unwrap();

        let mut expected = Vec::new();
        expected.extend_from_slice(SIGNED_OP.as_bytes());
        expected.extend_from_slice(&[0x5a; 32]);
        expected.extend_from_slice(&[0x03; 32]);
        expected
            .extend_from_slice(&xdrs::cxs::encode(&Grant::TYPE_DEF).unwrap());
        expected.extend_from_slice(
            &xdrs::cxs::encode(&BlockHeight::new(1_000)).unwrap(),
        );
        expected.extend_from_slice(&xdrs::cxs::encode(&1u64).unwrap());
        expected.extend_from_slice(&xdrs::cxs::encode(&body).unwrap());
        assert_eq!(preimage, expected);
    }

    /// Two payloads that encode to the same bytes must not produce the same
    /// message.
    #[test]
    fn type_def_separates_identically_encoded_bodies() {
        let grant = Grant { amount: 500 };
        let revoke = Revoke { id: 500 };
        assert_eq!(
            xdrs::cxs::encode(&grant).unwrap(),
            xdrs::cxs::encode(&revoke).unwrap(),
            "test is meaningless unless the values encode identically"
        );
        assert_ne!(
            op(&grant).preimage().unwrap(),
            op(&revoke).preimage().unwrap()
        );
    }

    #[test]
    fn adding_a_field_changes_the_message() {
        assert_eq!(Grant::TYPE_DEF.symbol(), GrantV2::TYPE_DEF.symbol());
        let v1 = Grant { amount: 7 };
        let v2 = GrantV2 {
            amount: 7,
            recipient: 0,
        };
        assert_ne!(op(&v1).preimage().unwrap(), op(&v2).preimage().unwrap());
    }

    #[test]
    fn each_header_field_changes_the_message() {
        let body = Grant { amount: 42 };
        let base = op(&body).preimage().unwrap();
        let variants = [
            SignedOp {
                network: NetworkHash::new(SHA256Hash::new([0xa5; 32])),
                ..op(&body)
            },
            SignedOp {
                verifying_contract: ContractAddress::from([0x04; 32]),
                ..op(&body)
            },
            SignedOp {
                uniquifier: Uniquifier {
                    block_expiry_height: BlockHeight::new(1_000),
                    nonce: 2,
                },
                ..op(&body)
            },
            SignedOp {
                uniquifier: Uniquifier {
                    block_expiry_height: BlockHeight::new(1_001),
                    nonce: 1,
                },
                ..op(&body)
            },
        ];
        for variant in variants {
            assert_ne!(base, variant.preimage().unwrap());
        }
    }
}
