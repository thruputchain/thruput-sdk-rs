//! The checks a verifying contract must perform, in one place.

use groundhog_domain_separator::{
    OperationTypeId, SignedOp as SignedOpPreimage, Syscall,
};
use alloc::vec::Vec;
use groundhog_xdr::types::NetworkHash;
use sdk::types::{ContractAddress, Ed25519PublicKey, Ed25519Signature};

use crate::{Operation, SignedOpError};

/// The exact bytes a signed operation's signature covers.
///
/// `network` and `verifying_contract` come from the *verifier's* knowledge, not
/// from `op` — see the crate docs on the network hash. `verifying_contract` is
/// normally `sdk::get_current_address()`.
///
/// Signed directly, with no pre-hash: Ed25519 hashes its own message, the same
/// call the transaction preimage makes. `groundhog-test-vectors signed-op` pins
/// these bytes against the off-chain implementation.
pub fn signed_op_preimage(
    network: &NetworkHash,
    verifying_contract: &ContractAddress,
    op: &Operation,
) -> Vec<u8> {
    preimage_of(network, verifying_contract, op).preimage()
}

fn preimage_of<'a>(
    network: &'a NetworkHash,
    verifying_contract: &'a ContractAddress,
    op: &'a Operation,
) -> SignedOpPreimage<'a> {
    SignedOpPreimage {
        network: network.clone(),
        verifying_contract: *verifying_contract,
        type_id: OperationTypeId::from_u64(op.type_id),
        nonce: op.nonce,
        valid_until: op.valid_until.clone(),
        body: op.body.0.as_slice(),
    }
}

/// A stable identifier for an operation: `sha256` of its
/// [`signed_op_preimage`]. Key nonce records or dedup logs with this; it is not
/// what gets signed.
pub fn signed_op_id(
    network: &NetworkHash,
    verifying_contract: &ContractAddress,
    op: &Operation,
) -> groundhog_xdr::types::SHA256Hash {
    preimage_of(network, verifying_contract, op).hash::<Syscall>()
}

/// Every check a verifier owes, in the order that fails cheapest first.
///
/// Throws the matching [`SignedOpError`] on the first failure and returns
/// normally if the operation is good; a caller that reaches the next line may
/// spend the nonce and mint tokens.
///
/// Spending the nonce is the *caller's* job, deliberately: this function takes
/// `&self`-shaped inputs only and touches no storage, so it stays usable from a
/// read-only entrypoint that wants to answer "would this be accepted?" without
/// consuming anything. A contract that calls this and then forgets to increment
/// the signer's nonce has built a replayable operation, so do both together.
///
/// # Example
///
/// ```ignore
/// let expected = self.nonces.get(&signer);
/// check_signed_op(
///     &self.network.get(),
///     &sdk::get_current_address(),
///     &op,
///     TRANSFER,
///     expected,
///     &signer_pubkey,
///     &signature,
/// );
/// self.nonces.set(&signer, expected + 1);
/// ```
#[allow(clippy::too_many_arguments)]
pub fn check_signed_op(
    network: &NetworkHash,
    verifying_contract: &ContractAddress,
    op: &Operation,
    expected_type: OperationTypeId,
    expected_nonce: u64,
    signer: &Ed25519PublicKey,
    signature: &Ed25519Signature,
) {
    // Type first: it is a plain integer compare, and getting it wrong means
    // the rest of the checks are being applied to the wrong kind of message.
    if op.type_id != expected_type.to_u64() {
        sdk::throw(SignedOpError::UnknownOperationType);
    }

    if op.nonce != expected_nonce {
        sdk::throw(SignedOpError::UnexpectedNonce);
    }

    // `valid_until` is inclusive: an operation is good through the end of that
    // height, matching how `Uniquifier::block_expiry_height` reads.
    if sdk::block::get_block_height() > op.valid_until {
        sdk::throw(SignedOpError::Expired);
    }

    // The signature last: it is the only expensive check, and by here the
    // network, verifying contract, type, nonce and expiry are all committed to
    // by the bytes it covers.
    let preimage = signed_op_preimage(network, verifying_contract, op);
    if !sdk::crypto::verify_signature_ed25519(signer, signature, &preimage) {
        sdk::throw(SignedOpError::InvalidSignature);
    }
}
