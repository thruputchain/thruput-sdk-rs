//! The checks a verifying contract performs, and the nonce state behind them.

use groundhog_contract_spec::ContractTypeSpec;
use groundhog_domain_separator::SignedOp;
use sdk::exn::NoneIfMissing;
use sdk::storage::{Map, Value};
use sdk::types::{Ed25519PublicKey, Ed25519Signature};
use sdk::xdr::{ReadXDR, WriteXDR};

use crate::{Operation, SignedOpError};

/// The stateless checks: expiry, decode, signature. Returns the decoded
/// payload, or throws the matching [`SignedOpError`].
///
/// This does not touch nonces, so on its own it does not make an operation
/// single-use. Use [`SignedOpNonces::verify_and_spend`] unless the contract
/// keeps its own replay state.
///
/// `T` is the payload type this contract expects. Its `TYPE_DEF` is part of
/// the signed bytes, so naming the wrong `T` fails as
/// [`SignedOpError::InvalidSignature`] rather than reinterpreting the message.
pub fn verify_signed_op<T>(op: &Operation, signature: &Ed25519Signature) -> T
where
    T: WriteXDR + for<'r> ReadXDR<'r> + ContractTypeSpec,
{
    // Inclusive, matching how `Uniquifier::block_expiry_height` reads.
    if sdk::block::get_block_height() > op.valid_until {
        sdk::throw(SignedOpError::Expired);
    }

    let body: T = match sdk::xdr::cxs::decode(op.body.0.as_slice()) {
        Ok(body) => body,
        Err(_) => sdk::throw(SignedOpError::MalformedBody),
    };

    let preimage = SignedOp {
        network: sdk::block::get_network_hash(),
        verifying_contract: sdk::get_current_address(),
        nonce: op.nonce,
        valid_until: op.valid_until.clone(),
        body: &body,
    }
    .preimage()
    .unwrap_or_else(|_| sdk::throw(SignedOpError::MalformedBody));

    if !sdk::crypto::verify_signature_ed25519(&op.signer, signature, &preimage)
    {
        sdk::throw(SignedOpError::InvalidSignature);
    }
    body
}

/// Per-signer sequence numbers for a signed-operation verifier.
///
/// Embed this in the contract's storage and drive it with
/// [`verify_and_spend`](Self::verify_and_spend), which checks and consumes the
/// nonce in one call so "verified but never spent" is not expressible.
///
/// Keyed by the signing key rather than an address: an operation is
/// authorized by whoever holds the key, and address-from-pubkey derivation
/// needs an account nonce this layer does not carry.
#[sdk::storage]
pub struct SignedOpNonces {
    next: Map<Ed25519PublicKey, Value<u64>>,
}

impl SignedOpNonces {
    /// The nonce `signer`'s next operation must carry.
    pub fn next_nonce(&self, signer: &Ed25519PublicKey) -> u64 {
        self.next
            .at(signer)
            .try_get()
            .none_if_missing()
            .unwrap_or(0)
    }

    /// Verify an operation and consume its nonce, returning the decoded
    /// payload. Throws [`SignedOpError`] on any failed check, in which case
    /// nothing is written.
    pub fn verify_and_spend<T>(
        &mut self,
        op: &Operation,
        signature: &Ed25519Signature,
    ) -> T
    where
        T: WriteXDR + for<'r> ReadXDR<'r> + ContractTypeSpec,
    {
        let expected = self.next_nonce(&op.signer);
        if op.nonce != expected {
            sdk::throw(SignedOpError::UnexpectedNonce);
        }
        let body = verify_signed_op::<T>(op, signature);
        self.next.at_mut(&op.signer).set(&(expected + 1));
        body
    }
}
