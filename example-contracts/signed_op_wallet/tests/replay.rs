//! The replay model, pinned: the chain's `(nonce, block_expiry_height)` pair,
//! consumed once at the verifying contract, dead after its expiry.
//!
//! Every transaction in a block executes against the previous block's state,
//! so each write below is followed by `advance_block` before anything reads
//! it, and "submit twice" is two tests: in one block it is a commit conflict,
//! across blocks it is a consumed pair.

use ed25519_dalek::{Signer, SigningKey};
use thruput_domain_separator::SignedOp as SignedOpPreimage;
use thruput_signed_op::{Operation, SignedOpClient, SignedOpError};
use thruput_sdk::signer::Signed;
use thruput_sdk::types::transaction::Uniquifier;
use thruput_sdk::types::{Ed25519PublicKey, Ed25519Signature};
use thruput_sdk::xdr::BytesM;
use thruput_sdk::*;
use signed_op_wallet::{Grant, SignedOpWalletClient};

const ALICE_SEED: [u8; 32] = [0x11; 32];
const MALLORY_SEED: [u8; 32] = [0x22; 32];
const AMOUNT: u64 = 42;
const NONCE: u64 = 0x5eed_0000_0000_0007;

fn wallet() -> ContractAddress {
    ContractAddress::named("wallet")
}
fn submitter() -> ContractAddress {
    ContractAddress::named("submitter")
}

fn public_key(seed: &[u8; 32]) -> Ed25519PublicKey {
    Ed25519PublicKey::from(
        SigningKey::from_bytes(seed).verifying_key().to_bytes(),
    )
}

fn setup() -> thruput_sdk::TestEnvironmentHandle {
    let env = thruput_sdk::TestEnvironment::no_cost();
    signed_op_wallet::register_mock_contract(&env)
        .deploy_contract_to(&wallet());
    env.transact(|tx| {
        tx.call(&submitter()).run(|| {
            SignedOpWalletClient::new(&wallet())
                .initialize(&public_key(&ALICE_SEED));
        })
    })
    .assert_success();
    env.advance_block();
    env
}

fn current_height(env: &thruput_sdk::TestEnvironmentHandle) -> BlockHeight {
    let result = env
        .transact(|tx| tx.call(&wallet()).run(thruput_sdk::block::get_block_height));
    result.assert_success();
    result.assert_return_as::<BlockHeight>()
}

/// What an off-chain signer does: build the preimage from the network, the
/// wallet, the uniquifier and the body's `TYPE_DEF`, then sign it.
fn sign(
    seed: &[u8; 32],
    verifying_contract: &ContractAddress,
    uniquifier: Uniquifier,
    body: &Grant,
) -> (Operation, Ed25519Signature) {
    let key = SigningKey::from_bytes(seed);
    let preimage = SignedOpPreimage {
        network: thruput_sdk::test_network_hash(),
        verifying_contract: *verifying_contract,
        uniquifier: uniquifier.clone(),
        body,
    }
    .preimage()
    .expect("encoding the signed-operation preimage");
    let op = Operation {
        block_expiry_height: uniquifier.block_expiry_height,
        nonce: uniquifier.nonce,
        body: BytesM(thruput_sdk::xdr::cxs::encode(body).expect("encoding the body")),
    };
    (op, Ed25519Signature::from(key.sign(&preimage).to_bytes()))
}

fn uniquifier(expiry: BlockHeight, nonce: u64) -> Uniquifier {
    Uniquifier {
        block_expiry_height: expiry,
        nonce,
    }
}

/// Alice's grant at `NONCE`, expiring `ahead` blocks from now.
fn alice_grant(
    env: &thruput_sdk::TestEnvironmentHandle,
    ahead: i64,
    nonce: u64,
) -> (Operation, Ed25519Signature) {
    let expiry = current_height(env) + ahead;
    sign(
        &ALICE_SEED,
        &wallet(),
        uniquifier(expiry, nonce),
        &Grant { amount: AMOUNT },
    )
}

/// Submit as `submitter()`, name the token's payload, and consume it here —
/// the submitter is the named consumer.
fn submit(
    env: &thruput_sdk::TestEnvironmentHandle,
    op: Operation,
    signature: Ed25519Signature,
) -> thruput_sdk::TransactResult {
    env.transact(|tx| {
        tx.call(&submitter()).run(move || {
            let token: Signed<u64> = SignedOpClient::new(&wallet())
                .execute_signed_op(op, &signature)
                .assume();
            let (amount, minter) = token.consume();
            assert_eq!(amount, AMOUNT);
            assert_eq!(minter, wallet());
        })
    })
}

/// Assert the wallet refuses `op` with `expected`. Inside the closure: a
/// `CallError` is not XDR-encodable and cannot leave the frame.
macro_rules! assert_refuses {
    ($env:expr, $op:expr, $signature:expr, $expected:expr $(,)?) => {{
        let (op, signature) = ($op, $signature);
        $env.transact(|tx| {
            tx.call(&submitter()).run(move || {
                let result = SignedOpClient::new(&wallet())
                    .try_execute_signed_op(op, &signature);
                assert_eq!(result.unwrap_err().try_as(), Some($expected));
            })
        })
        .assert_success();
    }};
}

fn prune(
    env: &thruput_sdk::TestEnvironmentHandle,
    u: &Uniquifier,
) -> thruput_sdk::TransactResult {
    let (expiry, nonce) = (u.block_expiry_height.clone(), u.nonce);
    env.transact(|tx| {
        tx.call(&submitter()).run(move || {
            SignedOpWalletClient::new(&wallet()).prune(expiry, nonce);
        })
    })
}

#[test]
fn a_signed_grant_mints_one_token_for_the_submitter() {
    let env = setup();
    let (op, sig) = alice_grant(&env, 8, NONCE);
    submit(&env, op, sig).assert_success();
}

#[test]
fn the_same_pair_cannot_be_consumed_again_in_a_later_block() {
    let env = setup();
    let (op, sig) = alice_grant(&env, 8, NONCE);
    submit(&env, op, sig).assert_success();
    env.advance_block();

    // Rebuilt, not cloned: ed25519 is deterministic, so these are the same
    // bytes a submitter would have kept.
    let (op, sig) = alice_grant(&env, 7, NONCE);
    assert_refuses!(&env, op, sig, SignedOpError::AlreadyConsumed);
}

/// Both submissions execute against the previous block's state, so both see
/// the pair absent and both verify. The exclusive lock on the consumed slot
/// makes the second commit conflict — the chain's rule for consuming one
/// uniquifier twice.
#[test]
fn the_same_pair_submitted_twice_in_one_block_loses_at_commit() {
    let env = setup();
    let (op, sig) = alice_grant(&env, 8, NONCE);
    submit(&env, op, sig).assert_success();
    let (op, sig) = alice_grant(&env, 8, NONCE);
    submit(&env, op, sig).assert_commit_failed_nondeterministically();
}

/// The pair is the identity, as for transactions: the same nonce under a
/// different expiry is a different operation and does not conflict.
#[test]
fn the_same_nonce_with_another_expiry_is_a_different_operation() {
    let env = setup();
    let (op, sig) = alice_grant(&env, 8, NONCE);
    submit(&env, op, sig).assert_success();
    env.advance_block();

    let (op, sig) = alice_grant(&env, 9, NONCE);
    submit(&env, op, sig).assert_success();
}

#[test]
fn nonces_need_no_order() {
    let env = setup();
    let (later, later_sig) = alice_grant(&env, 8, NONCE + 1);
    let (earlier, earlier_sig) = alice_grant(&env, 8, NONCE);
    submit(&env, later, later_sig).assert_success();
    env.advance_block();
    submit(&env, earlier, earlier_sig).assert_success();
}

#[test]
fn an_expired_operation_is_refused() {
    let env = setup();
    // Valid through the current height only; the next block is stale.
    let (op, sig) = alice_grant(&env, 0, NONCE);
    env.advance_block();
    assert_refuses!(&env, op, sig, SignedOpError::Expired);
}

/// Past its height a consumed pair is `Expired`, not `AlreadyConsumed`:
/// expiry is checked before the cache, so the refusal reads the same before
/// and after the entry is pruned.
#[test]
fn a_consumed_operation_reports_expired_once_past_its_height() {
    let env = setup();
    let u = uniquifier(current_height(&env) + 1, NONCE);
    let (op, sig) =
        sign(&ALICE_SEED, &wallet(), u.clone(), &Grant { amount: AMOUNT });
    submit(&env, op, sig).assert_success();
    env.advance_block();
    env.advance_block();

    let (op, sig) = sign(&ALICE_SEED, &wallet(), u, &Grant { amount: AMOUNT });
    assert_refuses!(&env, op, sig, SignedOpError::Expired);
}

/// Pruning drops a dead entry and can never re-admit the operation: the
/// expiry that makes the entry prunable is checked before the cache.
#[test]
fn pruning_waits_for_expiry_and_never_revives_an_operation() {
    let env = setup();
    let expiry = current_height(&env) + 1;
    let u = uniquifier(expiry.clone(), NONCE);
    let (op, sig) =
        sign(&ALICE_SEED, &wallet(), u.clone(), &Grant { amount: AMOUNT });
    submit(&env, op, sig).assert_success();
    env.advance_block();

    // Still within the expiry: the entry must stay.
    prune(&env, &u).assert_abort();
    env.advance_block();
    // Past it: gone.
    prune(&env, &u).assert_success();
    env.advance_block();
    // Pruning again is a no-op, and the operation itself is dead regardless.
    prune(&env, &u).assert_success();
    let (op, sig) = sign(&ALICE_SEED, &wallet(), u, &Grant { amount: AMOUNT });
    assert_refuses!(&env, op, sig, SignedOpError::Expired);
}

/// The wallet checks against the key it holds; the operation names no key,
/// so there is nothing on the wire for Mallory to substitute.
#[test]
fn a_signature_from_another_key_is_refused() {
    let env = setup();
    let expiry = current_height(&env) + 8;
    let (op, sig) = sign(
        &MALLORY_SEED,
        &wallet(),
        uniquifier(expiry, NONCE),
        &Grant { amount: AMOUNT },
    );
    assert_refuses!(&env, op, sig, SignedOpError::InvalidSignature);
}

#[test]
fn an_operation_signed_for_another_wallet_is_refused() {
    let env = setup();
    let expiry = current_height(&env) + 8;
    let (op, sig) = sign(
        &ALICE_SEED,
        &ContractAddress::named("some-other-wallet"),
        uniquifier(expiry, NONCE),
        &Grant { amount: AMOUNT },
    );
    assert_refuses!(&env, op, sig, SignedOpError::InvalidSignature);
}

#[test]
fn a_tampered_body_is_refused() {
    let env = setup();
    let (mut op, sig) = alice_grant(&env, 8, NONCE);
    let last = op.body.0.len() - 1;
    op.body.0[last] ^= 0x01;
    assert_refuses!(&env, op, sig, SignedOpError::InvalidSignature);
}
