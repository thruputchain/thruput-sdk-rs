//! The checks a verifying contract must perform, in one place.

use groundhog_contract_spec::ContractTypeSpec;
use groundhog_domain_separator::{SignedOp, Syscall};
use groundhog_xdr::types::{NetworkHash, SHA256Hash};
use sdk::types::{ContractAddress, Ed25519PublicKey, Ed25519Signature};
use sdk::xdr::{ReadXDR, WriteXDR};

use crate::{Operation, SignedOpError};

/// Rebuild the signed message from an [`Operation`] and its decoded payload.
///
/// `network` and `verifying_contract` come from the *verifier's* knowledge, not
/// from `op` — see the crate docs on the network hash. `verifying_contract` is
/// normally `sdk::get_current_address()`.
pub fn signed_op_of<'a, T: WriteXDR + ContractTypeSpec>(
    network: &NetworkHash,
    verifying_contract: &ContractAddress,
    op: &Operation,
    body: &'a T,
) -> SignedOp<'a, T> {
    SignedOp {
        network: network.clone(),
        verifying_contract: *verifying_contract,
        nonce: op.nonce,
        valid_until: op.valid_until.clone(),
        body,
    }
}

/// Decode an operation body as `T`, throwing
/// [`SignedOpError::MalformedBody`] if it does not.
pub fn decode_body<T>(op: &Operation) -> T
where
    T: for<'r> ReadXDR<'r>,
{
    match sdk::xdr::cxs::decode::<T>(op.body.0.as_slice()) {
        Ok(value) => value,
        Err(_) => sdk::throw(SignedOpError::MalformedBody),
    }
}

/// Every check a verifier owes, in the order that fails cheapest first.
///
/// Returns the decoded payload on success; a caller that reaches the next line
/// may spend the nonce and mint tokens. Throws the matching [`SignedOpError`]
/// on the first failure.
///
/// `T` is the payload type this contract expects. Its
/// [`ContractTypeSpec::TYPE_DEF`] is part of the signed bytes, so naming the
/// wrong `T` produces [`SignedOpError::InvalidSignature`] rather than silently
/// reinterpreting the message. There is no separate type check to skip.
///
/// Spending the nonce is the *caller's* job, deliberately: this touches no
/// storage, so it stays usable from a read-only entrypoint answering "would
/// this be accepted?". A contract that calls this and forgets to increment the
/// signer's nonce has built a replayable operation, so do both together.
///
/// # Example
///
/// ```ignore
/// let expected = self.nonces.get(&signer);
/// let transfer: Transfer = check_signed_op(
///     &self.network.get(),
///     &sdk::get_current_address(),
///     &op,
///     expected,
///     &signer_pubkey,
///     &signature,
/// );
/// self.nonces.set(&signer, expected + 1);
/// ```
pub fn check_signed_op<T>(
    network: &NetworkHash,
    verifying_contract: &ContractAddress,
    op: &Operation,
    expected_nonce: u64,
    signer: &Ed25519PublicKey,
    signature: &Ed25519Signature,
) -> T
where
    T: WriteXDR + for<'r> ReadXDR<'r> + ContractTypeSpec,
{
    if op.nonce != expected_nonce {
        sdk::throw(SignedOpError::UnexpectedNonce);
    }

    // `valid_until` is inclusive: an operation is good through the end of that
    // height, matching how `Uniquifier::block_expiry_height` reads.
    if sdk::block::get_block_height() > op.valid_until {
        sdk::throw(SignedOpError::Expired);
    }

    // Decoding before verifying is what lets the preimage be rebuilt from the
    // typed value rather than from the submitter's bytes. It also canonicalises
    // — a non-canonical encoding that happens to decode re-encodes to the form
    // the signer actually signed, or fails verification.
    let body: T = decode_body(op);

    // The signature last: it is the only expensive check, and by here the
    // network, verifying contract, payload type, nonce and expiry are all
    // committed to by the bytes it covers.
    let preimage =
        signed_op_of(network, verifying_contract, op, &body).preimage();
    if !sdk::crypto::verify_signature_ed25519(signer, signature, &preimage) {
        sdk::throw(SignedOpError::InvalidSignature);
    }

    body
}

/// A stable identifier for an operation: `sha256` of its signed bytes. Key
/// nonce records or dedup logs with this; it is not what gets signed.
pub fn signed_op_id<T: WriteXDR + ContractTypeSpec>(
    network: &NetworkHash,
    verifying_contract: &ContractAddress,
    op: &Operation,
    body: &T,
) -> SHA256Hash {
    signed_op_of(network, verifying_contract, op, body).hash::<Syscall>()
}
