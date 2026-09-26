//! Signed transfers through a wallet that verifies signatures and prevents replay.
//!
//! Transactions in a block read the previous block's state. These tests check
//! replay both at same-block commit and after the consumed nonce becomes visible.

use ed25519_dalek::{Signer, SigningKey};
use erc20_permit::{
    ERC20Client, ERC20PermitClient, ERC20PermitError, PermitGrant,
    PermitRequest,
};
use groundhog_domain_separator::SignedOp as SignedOpPreimage;
use groundhog_signed_op_if::{Operation, SignedOpClient, SignedOpError};
use permit_wallet::PermitWalletClient;
use sdk::signer::Signed;
use sdk::types::transaction::Uniquifier;
use sdk::types::{Ed25519PublicKey, Ed25519Signature};
use sdk::xdr::BytesM;
use sdk::*;

const ALICE_SEED: [u8; 32] = [0x11; 32];
const MALLORY_SEED: [u8; 32] = [0x22; 32];

const MINTED: u128 = 1_000;
const GRANTED: u128 = 250;

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

fn setup() -> sdk::TestEnvironmentHandle {
    let env = sdk::TestEnvironment::no_cost();

    permit_wallet::register_mock_contract(&env)
        .deploy_contract_to(&wallet_address());
    if let Some(path) = std::env::var_os("ERC20_PERMIT_RV32_ARTIFACT") {
        let contract = sdk::types::Contract::kRv32 {
            rv32_contract: sdk::types::contract::ContractBlob {
                data: BytesM(
                    std::fs::read(path).expect("read permit RV32 artifact"),
                ),
            },
        };
        env.setup_create_contract(&contract);
        env.setup_create_deployment(&token_address(), &contract);
        env.setup_create_deployment(&other_token_address(), &contract);
    } else {
        let token = erc20_permit::register_mock_contract(&env);
        token.deploy_contract_to(&token_address());
        token.deploy_contract_to(&other_token_address());
    }

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

fn current_height(env: &sdk::TestEnvironmentHandle) -> BlockHeight {
    let result = env.transact(|tx| {
        tx.call(&token_address()).run(sdk::block::get_block_height)
    });
    result.assert_success();
    result.assert_return_as::<BlockHeight>()
}

fn sign(
    seed: &[u8; 32],
    verifying_contract: &ContractAddress,
    nonce: u64,
    expiry: BlockHeight,
    body: &PermitRequest,
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

fn standard_request() -> PermitRequest {
    PermitRequest {
        token: token_address(),
        spender: spender(),
        amount: GRANTED,
    }
}

fn alice_transfer_until(expiry: BlockHeight) -> (Operation, Ed25519Signature) {
    sign(
        &ALICE_SEED,
        &wallet_address(),
        0,
        expiry,
        &standard_request(),
    )
}

fn alice_transfer(
    env: &sdk::TestEnvironmentHandle,
) -> (Operation, Ed25519Signature) {
    alice_transfer_until(current_height(env) + HORIZON)
}

fn mint_grant(
    op: Operation,
    signature: &Ed25519Signature,
) -> Signed<PermitGrant> {
    SignedOpClient::new(&wallet_address())
        .execute_signed_op(op, signature)
        .assume()
}

fn submit_transfer(
    env: &sdk::TestEnvironmentHandle,
    op: Operation,
    signature: Ed25519Signature,
) -> sdk::TransactResult {
    env.transact(|tx| {
        tx.call(&spender()).run(move || {
            let grant = mint_grant(op, &signature);
            ERC20PermitClient::new(&token_address())
                .transfer_from_signed(grant, &receiver());
        })
    })
}

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
fn transfer_from_signed_moves_the_balance_and_leaves_no_allowance() {
    let env = setup();
    let (op, signature) = alice_transfer_until(current_height(&env));

    env.transact(|tx| {
        tx.call(&spender()).run(move || {
            let grant = mint_grant(op, &signature);
            ERC20PermitClient::new(&token_address())
                .transfer_from_signed(grant, &receiver());
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
    let (op, signature) = alice_transfer(&env);

    // The grant is genuine — Alice signed it — but it names `spender()`, and
    // `receiver()` is the one presenting it.
    env.transact(|tx| {
        tx.call(&receiver()).run(move || {
            let grant = mint_grant(op, &signature);
            let result = ERC20PermitClient::new(&token_address())
                .try_transfer_from_signed(grant, &receiver());
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

#[test]
fn a_second_submission_in_the_same_block_loses_at_commit() {
    let env = setup();
    let expiry = current_height(&env) + HORIZON;

    let (op, signature) = alice_transfer_until(expiry.clone());
    submit_transfer(&env, op, signature).assert_success();

    let (op, signature) = alice_transfer_until(expiry);
    submit_transfer(&env, op, signature)
        .assert_commit_failed_nondeterministically();

    env.advance_block();
    assert_eq!(
        balance_of(&env, receiver()),
        GRANTED,
        "exactly one submission landed"
    );
}

#[test]
fn a_second_submission_in_a_later_block_is_already_consumed() {
    let env = setup();
    let expiry = current_height(&env) + HORIZON;

    let (op, signature) = alice_transfer_until(expiry.clone());
    submit_transfer(&env, op, signature).assert_success();
    env.advance_block();

    let (op, signature) = alice_transfer_until(expiry);
    assert_wallet_refuses!(
        &env,
        op,
        signature,
        SignedOpError::AlreadyConsumed
    );

    env.advance_block();
    assert_eq!(
        balance_of(&env, receiver()),
        GRANTED,
        "the replay added nothing"
    );
}

#[test]
fn a_signed_transfer_stops_working_after_its_expiry_height() {
    let env = setup();
    // Valid through the current height only, so the next block is stale.
    let (op, signature) = alice_transfer_until(current_height(&env));
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
fn a_transfer_signed_for_another_wallet_is_refused() {
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
    let (mut op, signature) = alice_transfer(&env);
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

#[test]
fn rotating_the_wallet_key_refuses_outstanding_signed_transfers() {
    let env = setup();
    let (op, signature) = alice_transfer(&env);

    rotate_owner_as(&env, admin(), public_key(&MALLORY_SEED)).assert_success();
    // The rotation is a write like any other: submitted in the same block,
    // the signed transfer would still see the old owner and succeed.
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

#[test]
fn a_transfer_from_a_former_key_revives_if_the_wallet_rotates_back() {
    let env = setup();
    let expiry = current_height(&env) + HORIZON;
    let (op, signature) = alice_transfer_until(expiry.clone());

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

    let (op, signature) = alice_transfer_until(expiry);
    submit_transfer(&env, op, signature).assert_success();
    env.advance_block();
    assert_eq!(balance_of(&env, receiver()), GRANTED);
}

#[test]
fn a_grant_minted_for_one_token_cannot_be_consumed_at_another() {
    let env = setup();
    // The body names `token_address()`, so the wallet mints a grant only that
    // contract may consume. Handing it to the other token is what a submitter
    // would try if the target were not signature-covered.
    let (op, signature) = alice_transfer(&env);

    let result = env.transact(|tx| {
        tx.call(&spender()).run(move || {
            let grant = mint_grant(op, &signature);
            ERC20PermitClient::new(&other_token_address())
                .transfer_from_signed(grant, &receiver());
        })
    });
    // Refused by `ConsumeAuthToken` inside the other token's frame — the
    // check lives in the runtime, not in either contract's code.
    result.assert_abort();
}

fn signed_amount(
    env: &sdk::TestEnvironmentHandle,
    nonce: u64,
    amount: u128,
) -> (Operation, Ed25519Signature) {
    sign(
        &ALICE_SEED,
        &wallet_address(),
        nonce,
        current_height(env) + HORIZON,
        &PermitRequest {
            token: token_address(),
            spender: spender(),
            amount,
        },
    )
}

#[test]
fn signed_transfer_preserves_an_existing_allowance() {
    let env = setup();
    env.transact(|tx| {
        tx.call(&wallet_address()).run(|| {
            ERC20Client::new(&token_address()).adjust_allowance(
                &spender(),
                100,
                false,
            );
        })
    })
    .assert_success();
    env.advance_block();
    let (op, signature) = alice_transfer(&env);
    let result = submit_transfer(&env, op, signature);
    result.assert_success();
    result.assert_event_count(1);
    result.assert_raw_event(0, |event| {
        let transfer = groundhog_erc20_if::TransferEvent::try_from(
            sdk::EmittedEvent::from(event.raw()),
        )
        .unwrap();
        assert_eq!(transfer.amount, GRANTED as i128);
    });
    env.advance_block();
    assert_eq!(balance_of(&env, receiver()), GRANTED);
    assert_eq!(allowance(&env), 100);
}

#[test]
fn invalid_transfer_amounts_roll_back_without_consuming_the_operation() {
    let env = setup();
    let expiry = current_height(&env) + HORIZON;
    for amount in [MINTED + 1, i128::MAX as u128 + 1] {
        let request = PermitRequest {
            amount,
            ..standard_request()
        };
        let (op, signature) =
            sign(&ALICE_SEED, &wallet_address(), 0, expiry.clone(), &request);
        let result = submit_transfer(&env, op, signature);
        assert_eq!(
            result.assert_abort_as::<groundhog_erc20_if::ERC20Error>(),
            groundhog_erc20_if::ERC20Error::TooLarge
        );
        result.assert_no_events();
        env.advance_block();
        assert_eq!(balance_of(&env, wallet_address()), MINTED);
        assert_eq!(allowance(&env), 0);
    }
    let (op, signature) = alice_transfer_until(expiry);
    submit_transfer(&env, op, signature).assert_success();
    env.advance_block();
    assert_eq!(balance_of(&env, receiver()), GRANTED);
}

#[test]
fn independent_signed_transfers_merge() {
    let env = setup();
    for nonce in [0, 1] {
        let (op, signature) = signed_amount(&env, nonce, GRANTED);
        submit_transfer(&env, op, signature).assert_success();
    }
    env.advance_block();
    assert_eq!(balance_of(&env, receiver()), 2 * GRANTED);
    assert_eq!(balance_of(&env, wallet_address()), MINTED - 2 * GRANTED);
    assert_eq!(allowance(&env), 0);
}

#[test]
fn a_transfer_signed_for_another_network_is_refused() {
    let env = setup();
    let expiry = current_height(&env) + HORIZON;
    let (op, _) = alice_transfer_until(expiry.clone());
    let preimage = SignedOpPreimage {
        network: sdk::types::NetworkHash::new(sdk::types::SHA256Hash::new(
            [0; 32],
        )),
        verifying_contract: wallet_address(),
        uniquifier: Uniquifier {
            block_expiry_height: expiry,
            nonce: 0,
        },
        body: &standard_request(),
    }
    .preimage()
    .unwrap();
    let signature = Ed25519Signature::from(
        SigningKey::from_bytes(&ALICE_SEED)
            .sign(&preimage)
            .to_bytes(),
    );
    assert_wallet_refuses!(
        &env,
        op,
        signature,
        SignedOpError::InvalidSignature
    );
    env.advance_block();
    assert_eq!(balance_of(&env, wallet_address()), MINTED);
}
