//! End-to-end permit: an owner signs off-chain, someone else submits.
//!
//! The submitter does two things in one frame: takes the signed operation to
//! the owner's wallet, which mints the grant token the request asks for, and
//! passes that token down into the ERC20. The owner's balance lives at the wallet's
//! address — the wallet *is* the account — so the identity the token acts on
//! is the address that minted the grant, and the assertions below are about
//! what a submitter can and cannot change about a signature they are merely
//! carrying.
//!
//! # Blocks, not sequence points
//!
//! Every transaction in a block executes against the state the *previous*
//! block left, and sees none of its neighbours' writes; conflicting writes
//! are settled at commit, where the loser fails nondeterministically. So each
//! write below is followed by `advance_block` before anything reads it, and
//! "submit the same permit twice" is two different tests: in one block it is
//! a commit conflict, across blocks it is a consumed pair.

use ed25519_dalek::{Signer, SigningKey};
use erc20_permit::{
    ClearAllowanceGrant, ClearAllowanceRequest, ERC20Client, ERC20Error,
    ERC20PermitClient, ERC20PermitError, PermitGrant, PermitRequest,
};
use groundhog_domain_separator::SignedOp as SignedOpPreimage;
use groundhog_signed_op_if::{Operation, SignedOpClient, SignedOpError};
use permit_wallet::{PermitWalletClient, WalletRequest};
use sdk::signer::Signed;
use sdk::types::transaction::Uniquifier;
use sdk::types::{Ed25519PublicKey, Ed25519Signature};
use sdk::xdr::BytesM;
use sdk::*;

const ALICE_SEED: [u8; 32] = [0x11; 32];
const MALLORY_SEED: [u8; 32] = [0x22; 32];

const MINTED: u128 = 1_000;
const GRANTED: u128 = 250;

/// How many blocks past signing a happy-path permit stays valid: enough to
/// survive the `advance_block`s a test needs to observe its own effects.
const HORIZON: i64 = 8;

fn wallet_address() -> ContractAddress {
    ContractAddress::named("alice-wallet")
}
fn token_address() -> ContractAddress {
    ContractAddress::named("permit-token")
}
fn other_token_address() -> ContractAddress {
    ContractAddress::named("other-token")
}
fn admin() -> ContractAddress {
    ContractAddress::named("admin")
}
fn spender() -> ContractAddress {
    ContractAddress::named("spender")
}
fn receiver() -> ContractAddress {
    ContractAddress::named("receiver")
}

fn public_key(seed: &[u8; 32]) -> Ed25519PublicKey {
    Ed25519PublicKey::from(
        SigningKey::from_bytes(seed).verifying_key().to_bytes(),
    )
}

/// A funded token at `token_address()`, a second at `other_token_address()`,
/// and Alice's wallet holding every minted unit — all visible, because the
/// block that wrote them has been advanced past.
fn setup() -> sdk::TestEnvironmentHandle {
    let env = sdk::TestEnvironment::no_cost();

    permit_wallet::register_mock_contract(&env)
        .deploy_contract_to(&wallet_address());
    let token = erc20_permit::register_mock_contract(&env);
    token.deploy_contract_to(&token_address());
    token.deploy_contract_to(&other_token_address());

    env.transact(|tx| {
        tx.call(&admin()).run(|| {
            PermitWalletClient::new(&wallet_address())
                .initialize(&admin(), &public_key(&ALICE_SEED));
            for token in [token_address(), other_token_address()] {
                let client = ERC20Client::new(&token);
                client.initialize(&admin());
                client.mint(&wallet_address(), MINTED);
            }
        })
    })
    .assert_success();
    env.advance_block();

    env
}

/// The current height, read from inside a frame because that is the only
/// place the syscall answers.
fn current_height(env: &sdk::TestEnvironmentHandle) -> BlockHeight {
    let result = env.transact(|tx| {
        tx.call(&token_address()).run(sdk::block::get_block_height)
    });
    result.assert_success();
    result.assert_return_as::<BlockHeight>()
}

/// Sign `body` as an operation addressed to `verifying_contract`.
///
/// Mirrors what an off-chain signer does: build the preimage from the network,
/// the wallet, the `(nonce, expiry)` uniquifier and the body's `TYPE_DEF`,
/// then sign it. The signer's key reaches neither contract: the wallet checks
/// against the key it holds.
fn sign(
    seed: &[u8; 32],
    verifying_contract: &ContractAddress,
    nonce: u64,
    expiry: BlockHeight,
    body: &WalletRequest,
) -> (Operation, Ed25519Signature) {
    let key = SigningKey::from_bytes(seed);
    let preimage = SignedOpPreimage {
        network: sdk::test_network_hash(),
        verifying_contract: *verifying_contract,
        uniquifier: Uniquifier {
            block_expiry_height: expiry.clone(),
            nonce,
        },
        body,
    }
    .preimage()
    .expect("encoding the signed-operation preimage");

    let op = Operation {
        block_expiry_height: expiry,
        nonce,
        body: BytesM(sdk::xdr::cxs::encode(body).expect("encoding the body")),
    };
    (op, Ed25519Signature::from(key.sign(&preimage).to_bytes()))
}

/// The permit Alice signs in most of these tests: `GRANTED` units of
/// `token_address()`, spendable by `spender()`.
fn standard_request() -> WalletRequest {
    WalletRequest::Permit(PermitRequest {
        token: token_address(),
        spender: spender(),
        amount: GRANTED,
    })
}

/// Alice's request to revoke `spender()`'s allowance on `token_address()`.
fn clear_request() -> WalletRequest {
    WalletRequest::ClearAllowance(ClearAllowanceRequest {
        token: token_address(),
        spender: spender(),
    })
}

/// Alice's standard permit at nonce 0, valid through `expiry`.
///
/// Rebuilt rather than cloned where a test needs it twice: ed25519 signing is
/// deterministic and the preimage is identical, so a second call with the
/// same expiry yields the same bytes a submitter would have kept a copy of.
fn alice_permit_until(expiry: BlockHeight) -> (Operation, Ed25519Signature) {
    sign(
        &ALICE_SEED,
        &wallet_address(),
        0,
        expiry,
        &standard_request(),
    )
}

/// Alice's standard permit, valid for `HORIZON` more blocks.
fn alice_permit(
    env: &sdk::TestEnvironmentHandle,
) -> (Operation, Ed25519Signature) {
    alice_permit_until(current_height(env) + HORIZON)
}

/// The submitter's first step: take the operation to Alice's wallet and name
/// the one token it mints as the grant. Runs inside a frame.
///
/// `T` is only what the submitter claims the grant is: the wallet returns it
/// untyped, and the token contract decodes the payload when it consumes it.
fn mint_grant<T: sdk::mintable::MintableType>(
    op: Operation,
    signature: &Ed25519Signature,
) -> Signed<T> {
    SignedOpClient::new(&wallet_address())
        .execute_signed_op(op, signature)
        .assume()
}

/// Submit `op` as `spender()`: mint the grant at the wallet, then pass it into
/// `token_address()`'s `increase_allowance_signed` — the whole flow, in one
/// frame.
fn submit_permit(
    env: &sdk::TestEnvironmentHandle,
    op: Operation,
    signature: Ed25519Signature,
) -> sdk::TransactResult {
    env.transact(|tx| {
        tx.call(&spender()).run(move || {
            let grant = mint_grant::<PermitGrant>(op, &signature);
            ERC20PermitClient::new(&token_address())
                .increase_allowance_signed(grant);
        })
    })
}

/// Assert that the wallet refuses `op` with `expected`.
///
/// A macro rather than a function returning the error: a `CallError` is not
/// XDR-encodable, so it cannot leave the frame and the check has to happen
/// inside the closure. These cases never reach the token contract: a refused
/// operation mints nothing to pass down.
macro_rules! assert_wallet_refuses {
    ($env:expr, $op:expr, $signature:expr, $expected:expr $(,)?) => {{
        let (op, signature) = ($op, $signature);
        $env.transact(|tx| {
            tx.call(&spender()).run(move || {
                let result = SignedOpClient::new(&wallet_address())
                    .try_execute_signed_op(op, &signature);
                assert_eq!(result.unwrap_err().try_as(), Some($expected));
            })
        })
        .assert_success();
    }};
}

fn balance_of(env: &sdk::TestEnvironmentHandle, who: ContractAddress) -> u128 {
    let result = env.transact(|tx| {
        tx.call(&admin()).run(move || {
            ERC20Client::new(&token_address()).get_balance_of(&who)
        })
    });
    result.assert_success();
    result.assert_return_as::<u128>()
}

fn allowance(env: &sdk::TestEnvironmentHandle) -> u128 {
    let result = env.transact(|tx| {
        tx.call(&admin()).run(|| {
            ERC20Client::new(&token_address())
                .allowance(&wallet_address(), &spender())
        })
    });
    result.assert_success();
    result.assert_return_as::<u128>()
}

fn wallet_owner(env: &sdk::TestEnvironmentHandle) -> Ed25519PublicKey {
    let result = env.transact(|tx| {
        tx.call(&admin())
            .run(|| PermitWalletClient::new(&wallet_address()).owner())
    });
    result.assert_success();
    result.assert_return_as::<Ed25519PublicKey>()
}

/// Rotate the wallet's owner as `caller`, and hand back the result.
fn rotate_owner_as(
    env: &sdk::TestEnvironmentHandle,
    caller: ContractAddress,
    new_owner: Ed25519PublicKey,
) -> sdk::TransactResult {
    env.transact(|tx| {
        tx.call(&caller).run(move || {
            PermitWalletClient::new(&wallet_address())
                .rotate_owner(&new_owner);
        })
    })
}

#[test]
fn increase_allowance_signed_records_an_allowance_the_spender_can_draw_on() {
    let env = setup();
    let (op, signature) = alice_permit(&env);

    // Submitted by the spender, but it would work from any address: the
    // authority comes from the signature, not from who carries it.
    submit_permit(&env, op, signature).assert_success();
    env.advance_block();
    assert_eq!(allowance(&env), GRANTED);

    env.transact(|tx| {
        tx.call(&spender()).run(|| {
            ERC20Client::new(&token_address()).transfer_from(
                &wallet_address(),
                &receiver(),
                GRANTED,
            );
        })
    })
    .assert_success();
    env.advance_block();

    assert_eq!(balance_of(&env, receiver()), GRANTED);
    assert_eq!(balance_of(&env, wallet_address()), MINTED - GRANTED);
    assert_eq!(allowance(&env), 0);
}

#[test]
fn transfer_from_signed_moves_the_balance_and_leaves_no_allowance() {
    let env = setup();
    let (op, signature) = alice_permit(&env);

    env.transact(|tx| {
        tx.call(&spender()).run(move || {
            let grant = mint_grant(op, &signature);
            ERC20PermitClient::new(&token_address()).transfer_from_signed(
                grant,
                &receiver(),
                GRANTED,
            );
        })
    })
    .assert_success();
    env.advance_block();

    assert_eq!(balance_of(&env, receiver()), GRANTED);
    assert_eq!(balance_of(&env, wallet_address()), MINTED - GRANTED);
    // The whole point of this path: the capability was spent, not copied into
    // storage where it would outlive the transfer.
    assert_eq!(allowance(&env), 0);
}

#[test]
fn only_the_named_spender_may_redeem_a_grant_directly() {
    let env = setup();
    let (op, signature) = alice_permit(&env);

    // The grant is genuine — Alice signed it — but it names `spender()`, and
    // `receiver()` is the one presenting it.
    env.transact(|tx| {
        tx.call(&receiver()).run(move || {
            let grant = mint_grant(op, &signature);
            let result = ERC20PermitClient::new(&token_address())
                .try_transfer_from_signed(grant, &receiver(), GRANTED);
            assert_eq!(
                result.unwrap_err().try_as(),
                Some(ERC20PermitError::NotSpender),
            );
        })
    })
    .assert_success();
    env.advance_block();

    assert_eq!(balance_of(&env, receiver()), 0);
}

/// Within one block, replay is settled by the commit protocol, not the cache.
///
/// Both submissions execute against the previous block's state, so both see
/// the pair unconsumed, both verify, and both would write it. Identical
/// values commit side by side — which is why `consume` takes the slot under
/// an *exclusive* lock: the second commit conflicts instead of landing.
#[test]
fn a_second_submission_in_the_same_block_loses_at_commit() {
    let env = setup();
    let expiry = current_height(&env) + HORIZON;

    let (op, signature) = alice_permit_until(expiry.clone());
    submit_permit(&env, op, signature).assert_success();

    let (op, signature) = alice_permit_until(expiry);
    submit_permit(&env, op, signature)
        .assert_commit_failed_nondeterministically();

    env.advance_block();
    assert_eq!(allowance(&env), GRANTED, "exactly one submission landed");
}

/// Across blocks, replay is settled by the cache: the first consumption of
/// the `(nonce, block_expiry_height)` pair is visible, and the same pair is
/// refused.
#[test]
fn a_second_submission_in_a_later_block_is_already_consumed() {
    let env = setup();
    let expiry = current_height(&env) + HORIZON;

    let (op, signature) = alice_permit_until(expiry.clone());
    submit_permit(&env, op, signature).assert_success();
    env.advance_block();

    let (op, signature) = alice_permit_until(expiry);
    assert_wallet_refuses!(
        &env,
        op,
        signature,
        SignedOpError::AlreadyConsumed
    );

    env.advance_block();
    assert_eq!(allowance(&env), GRANTED, "the replay added nothing");
}

#[test]
fn a_permit_stops_working_after_its_expiry_height() {
    let env = setup();
    // Valid through the current height only, so the next block is stale.
    let (op, signature) = alice_permit_until(current_height(&env));
    env.advance_block();

    assert_wallet_refuses!(&env, op, signature, SignedOpError::Expired);

    env.advance_block();
    assert_eq!(allowance(&env), 0);
}

#[test]
fn a_signature_from_another_key_is_refused() {
    let env = setup();
    let expiry = current_height(&env) + HORIZON;
    let (op, signature) = sign(
        &MALLORY_SEED,
        &wallet_address(),
        0,
        expiry,
        &standard_request(),
    );

    // The wallet checks against the key it holds, and nothing on the wire
    // names Mallory's, so her signature simply does not verify.
    assert_wallet_refuses!(
        &env,
        op,
        signature,
        SignedOpError::InvalidSignature
    );

    env.advance_block();
    assert_eq!(allowance(&env), 0);
}

#[test]
fn a_permit_signed_for_another_wallet_is_refused() {
    let env = setup();
    let expiry = current_height(&env) + HORIZON;
    // Same key, same body, same uniquifier — only the wallet the preimage
    // names differs, which is what stops one wallet's signature being
    // replayed at another.
    let (op, signature) = sign(
        &ALICE_SEED,
        &ContractAddress::named("some-other-wallet"),
        0,
        expiry,
        &standard_request(),
    );

    assert_wallet_refuses!(
        &env,
        op,
        signature,
        SignedOpError::InvalidSignature
    );

    env.advance_block();
    assert_eq!(allowance(&env), 0);
}

#[test]
fn a_tampered_body_is_refused() {
    let env = setup();
    let (mut op, signature) = alice_permit(&env);
    // Raise the amount on the wire. The body is covered by the preimage, so
    // the signature no longer matches what the wallet re-derives.
    let last = op.body.0.len() - 1;
    op.body.0[last] ^= 0x01;

    assert_wallet_refuses!(
        &env,
        op,
        signature,
        SignedOpError::InvalidSignature
    );

    env.advance_block();
    assert_eq!(allowance(&env), 0);
}

#[test]
fn only_the_administrator_may_rotate_the_owner() {
    let env = setup();

    let result = rotate_owner_as(&env, spender(), public_key(&MALLORY_SEED));
    result.assert_abort();
    env.advance_block();

    assert_eq!(wallet_owner(&env), public_key(&ALICE_SEED));
}

/// Alice signs, then the administrator rotates her key before anyone submits.
///
/// This is the case a design that derives the owner from the signing key gets
/// wrong: the address would still derive from the old key, so the stale
/// permit would be honored. Here authority is wallet state, the rotation is
/// one write, and Alice's signature no longer verifies under the key the
/// wallet now checks against.
#[test]
fn rotating_the_wallet_key_refuses_outstanding_permits() {
    let env = setup();
    let (op, signature) = alice_permit(&env);

    rotate_owner_as(&env, admin(), public_key(&MALLORY_SEED)).assert_success();
    // The rotation is a write like any other: submitted in the same block,
    // the permit would still see the old owner and succeed.
    env.advance_block();

    assert_wallet_refuses!(
        &env,
        op,
        signature,
        SignedOpError::InvalidSignature
    );

    env.advance_block();
    assert_eq!(allowance(&env), 0);
    assert_eq!(balance_of(&env, wallet_address()), MINTED);
}

/// Refusal is a throw, a throw reverts the frame, and a reverted frame writes
/// nothing — so a refused permit does not consume its pair. Rotate back to
/// the key that signed it and the very same bytes are valid again. "Refused
/// while rotated out" is the guarantee; "retired" would be a stronger claim
/// than the interface can make, since it requires refusal to throw.
#[test]
fn a_permit_from_a_former_key_revives_if_the_wallet_rotates_back() {
    let env = setup();
    let expiry = current_height(&env) + HORIZON;
    let (op, signature) = alice_permit_until(expiry.clone());

    rotate_owner_as(&env, admin(), public_key(&MALLORY_SEED)).assert_success();
    env.advance_block();
    assert_wallet_refuses!(
        &env,
        op,
        signature,
        SignedOpError::InvalidSignature
    );
    env.advance_block();

    rotate_owner_as(&env, admin(), public_key(&ALICE_SEED)).assert_success();
    env.advance_block();

    let (op, signature) = alice_permit_until(expiry);
    submit_permit(&env, op, signature).assert_success();
    env.advance_block();
    assert_eq!(allowance(&env), GRANTED);
}

#[test]
fn a_grant_minted_for_one_token_cannot_be_consumed_at_another() {
    let env = setup();
    // The body names `token_address()`, so the wallet mints a grant only that
    // contract may consume. Handing it to the other token is what a submitter
    // would try if the target were not signature-covered.
    let (op, signature) = alice_permit(&env);

    let result = env.transact(|tx| {
        tx.call(&spender()).run(move || {
            let grant = mint_grant::<PermitGrant>(op, &signature);
            ERC20PermitClient::new(&other_token_address())
                .increase_allowance_signed(grant);
        })
    });
    // Refused by `ConsumeAuthToken` inside the other token's frame — the
    // check lives in the runtime, not in either contract's code.
    result.assert_abort();
}

/// Submit a signed `body` from Alice at `nonce` as `spender()`, redeeming it
/// through `redeem` inside the same frame.
fn submit_signed<F>(
    env: &sdk::TestEnvironmentHandle,
    nonce: u64,
    body: WalletRequest,
    redeem: F,
) -> sdk::TransactResult
where
    F: FnOnce(Operation, Ed25519Signature) + std::panic::UnwindSafe + 'static,
{
    let (op, signature) = sign(
        &ALICE_SEED,
        &wallet_address(),
        nonce,
        current_height(env) + HORIZON,
        &body,
    );
    env.transact(|tx| tx.call(&spender()).run(move || redeem(op, signature)))
}

/// A grant is a ceiling, not an exact amount: the spender moves what it needs,
/// and the unused remainder goes nowhere.
#[test]
fn transfer_from_signed_moves_up_to_the_granted_amount() {
    let env = setup();
    submit_signed(&env, 0, standard_request(), |op, signature| {
        let grant = mint_grant(op, &signature);
        ERC20PermitClient::new(&token_address()).transfer_from_signed(
            grant,
            &receiver(),
            GRANTED - 50,
        );
    })
    .assert_success();
    env.advance_block();

    assert_eq!(balance_of(&env, receiver()), GRANTED - 50);
    assert_eq!(balance_of(&env, wallet_address()), MINTED - GRANTED + 50);
    assert_eq!(allowance(&env), 0, "the remainder is not kept as allowance");
}

#[test]
fn transfer_from_signed_refuses_more_than_the_grant() {
    let env = setup();
    submit_signed(&env, 0, standard_request(), |op, signature| {
        let grant = mint_grant(op, &signature);
        let result = ERC20PermitClient::new(&token_address())
            .try_transfer_from_signed(grant, &receiver(), GRANTED + 1);
        assert_eq!(
            result.unwrap_err().try_as(),
            Some(ERC20PermitError::ExceedsGrant),
        );
    })
    .assert_success();
    env.advance_block();

    assert_eq!(balance_of(&env, receiver()), 0);
}

/// Within the grant, the owner's balance still bounds the transfer.
#[test]
fn transfer_from_signed_is_bounded_by_the_owners_balance() {
    let env = setup();
    let body = WalletRequest::Permit(PermitRequest {
        token: token_address(),
        spender: spender(),
        amount: MINTED + 1,
    });
    submit_signed(&env, 0, body, |op, signature| {
        let grant = mint_grant(op, &signature);
        let result = ERC20PermitClient::new(&token_address())
            .try_transfer_from_signed(grant, &receiver(), MINTED + 1);
        assert_eq!(result.unwrap_err().try_as(), Some(ERC20Error::TooLarge));
    })
    .assert_success();
    env.advance_block();

    assert_eq!(balance_of(&env, wallet_address()), MINTED);
}

/// Two grants for the same spender sum: the method increases, it never sets.
#[test]
fn increase_allowance_signed_adds_to_the_existing_allowance() {
    let env = setup();
    for nonce in [0, 1] {
        submit_signed(&env, nonce, standard_request(), |op, signature| {
            let grant = mint_grant::<PermitGrant>(op, &signature);
            ERC20PermitClient::new(&token_address())
                .increase_allowance_signed(grant);
        })
        .assert_success();
        env.advance_block();
    }

    assert_eq!(allowance(&env), 2 * GRANTED);
}

#[test]
fn clear_allowance_signed_revokes_a_standing_allowance() {
    let env = setup();
    submit_signed(&env, 0, standard_request(), |op, signature| {
        let grant = mint_grant::<PermitGrant>(op, &signature);
        ERC20PermitClient::new(&token_address())
            .increase_allowance_signed(grant);
    })
    .assert_success();
    env.advance_block();
    assert_eq!(allowance(&env), GRANTED);

    submit_signed(&env, 1, clear_request(), |op, signature| {
        let grant = mint_grant::<ClearAllowanceGrant>(op, &signature);
        ERC20PermitClient::new(&token_address()).clear_allowance_signed(grant);
    })
    .assert_success();
    env.advance_block();
    assert_eq!(allowance(&env), 0);

    // The revoked allowance can no longer be drawn on.
    env.transact(|tx| {
        tx.call(&spender()).run(|| {
            let result = ERC20Client::new(&token_address()).try_transfer_from(
                &wallet_address(),
                &receiver(),
                1,
            );
            assert_eq!(
                result.unwrap_err().try_as(),
                Some(ERC20Error::TooLarge)
            );
        })
    })
    .assert_success();
}

/// The signed body names the operation, and the grant's payload follows it: a
/// grant signed to raise an allowance cannot be redeemed to clear one, and a
/// clear cannot be redeemed as a transfer. The token contract refuses to
/// decode the payload as the other type, so the call aborts.
#[test]
fn a_grant_cannot_be_redeemed_for_a_different_operation() {
    let env = setup();
    submit_signed(&env, 0, standard_request(), |op, signature| {
        let grant = mint_grant::<ClearAllowanceGrant>(op, &signature);
        ERC20PermitClient::new(&token_address()).clear_allowance_signed(grant);
    })
    .assert_abort();
    env.advance_block();

    submit_signed(&env, 1, clear_request(), |op, signature| {
        let grant = mint_grant::<PermitGrant>(op, &signature);
        ERC20PermitClient::new(&token_address()).transfer_from_signed(
            grant,
            &receiver(),
            0,
        );
    })
    .assert_abort();
    env.advance_block();

    assert_eq!(allowance(&env), 0);
    assert_eq!(balance_of(&env, receiver()), 0);
}

/// The clear deletes under an exclusive lock, so an increase submitted in the
/// same block cannot land beside it: one of the two commits, the other fails
/// at commit, and the allowance is never left holding the increase on top of
/// a clear that did not see it.
#[test]
fn a_clear_conflicts_with_an_increase_in_the_same_block() {
    let env = setup();
    submit_signed(&env, 0, standard_request(), |op, signature| {
        let grant = mint_grant::<PermitGrant>(op, &signature);
        ERC20PermitClient::new(&token_address())
            .increase_allowance_signed(grant);
    })
    .assert_success();
    env.advance_block();

    submit_signed(&env, 1, clear_request(), |op, signature| {
        let grant = mint_grant::<ClearAllowanceGrant>(op, &signature);
        ERC20PermitClient::new(&token_address()).clear_allowance_signed(grant);
    })
    .assert_success();
    submit_signed(&env, 2, standard_request(), |op, signature| {
        let grant = mint_grant::<PermitGrant>(op, &signature);
        ERC20PermitClient::new(&token_address())
            .increase_allowance_signed(grant);
    })
    .assert_commit_failed_nondeterministically();
    env.advance_block();

    assert_eq!(allowance(&env), 0, "only the clear landed");
}
