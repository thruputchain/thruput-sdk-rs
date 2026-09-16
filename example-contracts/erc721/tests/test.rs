//! Host-side tests for the ERC721 example: minting, transfers under each
//! kind of authorization, approvals, safe transfers against receivers that
//! accept, reject, or do not exist, burning, and ERC-165 detection.

use erc721::{NftAdminClient, NftError};
use erc721_receiver::{Delivery, REJECT_DATA, ReceiverClient, ReceiverError};
use groundhog_erc165_if::{ERC165Client, try_probe_interface};
use groundhog_erc721_if::{
    ApprovalEvent, ApprovalForAllEvent, BurnEvent, CONTRACT_ADDR_ZERO,
    ERC721Client, ERC721Entrypoints, ERC721Error, ERC721MetadataClient,
    ERC721MetadataEntrypoints, ERC721ReceiverEntrypoints, MintEvent,
    OnchainMetadata, TokenId, TokenMetadata, TransferData, TransferEvent,
};
use sdk::call::CallError;
use sdk::*;

hash_ident! { static TOKEN_NONCE: SHA256Hash; }
hash_ident! { static RECEIVER_NONCE: SHA256Hash; }
hash_ident! { static COUNTER_NONCE: SHA256Hash; }
hash_ident! { static ADMIN: ContractAddress; }
hash_ident! { static ALICE: ContractAddress; }
hash_ident! { static BOB: ContractAddress; }
hash_ident! { static OPERATOR: ContractAddress; }
hash_ident! { static NOBODY: ContractAddress; }

const NAME: &str = "Groundhog Punks";
const SYMBOL: &str = "GHP";
const BASE_URI: &str = "https://example.com/nft/";

/// A token id with `n` in its last byte.
fn id(n: u8) -> TokenId {
    let mut bytes = [0u8; 32];
    bytes[31] = n;
    TokenId::new(bytes)
}

/// Off-chain metadata for token `n`, at the collection's base URI.
fn uri_metadata(n: u8) -> TokenMetadata {
    TokenMetadata::Uri(format!("{BASE_URI}{n}").as_str().try_into().unwrap())
}

/// Metadata stored on chain.
fn onchain_metadata() -> TokenMetadata {
    TokenMetadata::Onchain(OnchainMetadata {
        name: "Punk with every bit set".try_into().unwrap(),
        description: "Its id is thirty-two 0xff bytes.".try_into().unwrap(),
        image: "data:image/svg+xml,<svg/>".try_into().unwrap(),
    })
}

/// Deploy the token, initialize it as `ADMIN`, and mint ids 1 and 2 to
/// `ALICE`.
fn deploy_token(env: &TestEnvironmentHandle) -> ContractAddress {
    let contract = erc721::register_mock_contract(env);
    let hash = *contract.as_hash();

    let result = env.transact(|tx| {
        tx.call(&ADMIN).run(move || {
            let addr = deploy_contract(&TOKEN_NONCE, &hash);
            let admin = NftAdminClient::new(&addr);
            admin.initialize(
                &ADMIN,
                NAME.try_into().unwrap(),
                SYMBOL.try_into().unwrap(),
            );
            admin.mint(&ALICE, &id(1), uri_metadata(1));
            admin.mint(&ALICE, &id(2), uri_metadata(2));
            addr
        })
    });
    result.assert_success();
    env.advance_block();
    result.assert_return_as()
}

/// Deploy the receiver fixture and a contract with no receiver hook.
fn deploy_destinations(
    env: &TestEnvironmentHandle,
) -> (ContractAddress, ContractAddress) {
    let receiver_hash =
        *erc721_receiver::register_mock_contract(env).as_hash();
    let counter_hash = *counter::register_mock_contract(env).as_hash();

    let result = env.transact(|tx| {
        tx.call(&ADMIN).run(move || {
            (
                deploy_contract(&RECEIVER_NONCE, &receiver_hash),
                deploy_contract(&COUNTER_NONCE, &counter_hash),
            )
        })
    });
    result.assert_success();
    env.advance_block();
    result.assert_return_as()
}

fn transfers(
    result: &TransactResult,
    token: &ContractAddress,
) -> Vec<(ContractAddress, ContractAddress, TokenId)> {
    result
        .events_from(token)
        .into_iter()
        .filter_map(|e| TransferEvent::try_from(EmittedEvent::from(e)).ok())
        .map(|e| (*e.from, *e.to, e.token_id))
        .collect()
}

fn mints(
    result: &TransactResult,
    token: &ContractAddress,
) -> Vec<(ContractAddress, TokenId)> {
    result
        .events_from(token)
        .into_iter()
        .filter_map(|e| MintEvent::try_from(EmittedEvent::from(e)).ok())
        .map(|e| (*e.to, e.token_id))
        .collect()
}

fn burns(
    result: &TransactResult,
    token: &ContractAddress,
) -> Vec<(ContractAddress, TokenId)> {
    result
        .events_from(token)
        .into_iter()
        .filter_map(|e| BurnEvent::try_from(EmittedEvent::from(e)).ok())
        .map(|e| (*e.from, e.token_id))
        .collect()
}

fn approvals(
    result: &TransactResult,
    token: &ContractAddress,
) -> Vec<(ContractAddress, ContractAddress, TokenId)> {
    result
        .events_from(token)
        .into_iter()
        .filter_map(|e| ApprovalEvent::try_from(EmittedEvent::from(e)).ok())
        .map(|e| (*e.owner, *e.approved, e.token_id))
        .collect()
}

fn approvals_for_all(
    result: &TransactResult,
    token: &ContractAddress,
) -> Vec<(ContractAddress, ContractAddress, bool)> {
    result
        .events_from(token)
        .into_iter()
        .filter_map(|e| {
            ApprovalForAllEvent::try_from(EmittedEvent::from(e)).ok()
        })
        .map(|e| (*e.owner, *e.operator, e.approved))
        .collect()
}

#[test]
fn mint_sets_owner_balance_metadata_and_events() {
    let env = TestEnvironment::no_cost();
    let contract = erc721::register_mock_contract(&env);
    let hash = *contract.as_hash();
    let big = TokenId::new([0xff; 32]);

    let result = env.transact(|tx| {
        tx.call(&ADMIN).run(move || {
            let addr = deploy_contract(&TOKEN_NONCE, &hash);
            let admin = NftAdminClient::new(&addr);
            admin.initialize(
                &ADMIN,
                NAME.try_into().unwrap(),
                SYMBOL.try_into().unwrap(),
            );
            assert_eq!(
                admin
                    .try_initialize(
                        &ADMIN,
                        NAME.try_into().unwrap(),
                        SYMBOL.try_into().unwrap(),
                    )
                    .unwrap_err()
                    .try_as(),
                Some(NftError::AlreadyInitialized),
            );

            admin.mint(&ALICE, &id(1), uri_metadata(1));
            admin.mint(&BOB, &big, onchain_metadata());
            assert_eq!(
                admin
                    .try_mint(&BOB, &id(1), uri_metadata(1))
                    .unwrap_err()
                    .try_as(),
                Some(NftError::TokenAlreadyMinted),
            );

            let token = ERC721Client::new(&addr);
            assert_eq!(token.owner_of(&id(1)), ALICE);
            assert_eq!(token.owner_of(&big), BOB);
            assert_eq!(token.balance_of(&ALICE), 1);
            assert_eq!(token.balance_of(&BOB), 1);
            assert_eq!(token.balance_of(&OPERATOR), 0);
            assert_eq!(token.get_approved(&id(1)), *CONTRACT_ADDR_ZERO);

            let meta = ERC721MetadataClient::new(&addr);
            assert_eq!(meta.name().as_str(), NAME);
            assert_eq!(meta.symbol().as_str(), SYMBOL);
            // Metadata comes back as minted; only the off-chain kind has
            // a URI to give.
            assert_eq!(
                meta.token_uri(&id(1)).as_str(),
                "https://example.com/nft/1"
            );
            assert_eq!(meta.token_metadata(&id(1)), uri_metadata(1));
            assert_eq!(meta.token_metadata(&big), onchain_metadata());
            assert_eq!(
                meta.try_token_uri(&big).unwrap_err().try_as(),
                Some(ERC721Error::NoTokenUri)
            );

            // Everything about an unminted id is `NonexistentToken`.
            let missing = |r: Result<(), CallError>| {
                assert_eq!(
                    r.unwrap_err().try_as(),
                    Some(ERC721Error::NonexistentToken)
                );
            };
            missing(token.try_owner_of(&id(9)).map(drop));
            missing(token.try_get_approved(&id(9)).map(drop));
            missing(meta.try_token_uri(&id(9)).map(drop));
            missing(meta.try_token_metadata(&id(9)).map(drop));
            missing(token.try_transfer_from(&ALICE, &BOB, &id(9)));
            addr
        })
    });
    result.assert_success();
    let addr: ContractAddress = result.assert_return_as();
    assert_eq!(mints(&result, &addr), vec![(ALICE, id(1)), (BOB, big)]);
    assert!(transfers(&result, &addr).is_empty());
    env.advance_block();

    // Only the administrator mints.
    let result = env.transact(|tx| {
        tx.call(&ALICE).run(move || {
            assert_eq!(
                NftAdminClient::new(&addr)
                    .try_mint(&ALICE, &id(3), uri_metadata(3))
                    .unwrap_err()
                    .try_as(),
                Some(NftError::MustBeAdministrator),
            );
        })
    });
    result.assert_success();
}

#[test]
fn transfer_from_checks_owner_and_authorization() {
    let env = TestEnvironment::no_cost();
    let addr = deploy_token(&env);

    // A stranger cannot move Alice's token.
    let result = env.transact(|tx| {
        tx.call(&BOB).run(move || {
            let token = ERC721Client::new(&addr);
            assert_eq!(
                token
                    .try_transfer_from(&ALICE, &BOB, &id(1))
                    .unwrap_err()
                    .try_as(),
                Some(ERC721Error::InsufficientApproval),
            );
            assert_eq!(token.owner_of(&id(1)), ALICE);
        })
    });
    result.assert_success();
    env.advance_block();

    // The owner can, but must name themselves as `from`.
    let result = env.transact(|tx| {
        tx.call(&ALICE).run(move || {
            let token = ERC721Client::new(&addr);
            assert_eq!(
                token
                    .try_transfer_from(&BOB, &ALICE, &id(1))
                    .unwrap_err()
                    .try_as(),
                Some(ERC721Error::IncorrectOwner),
            );

            token.transfer_from(&ALICE, &BOB, &id(1));
            assert_eq!(token.owner_of(&id(1)), BOB);
            assert_eq!(token.balance_of(&ALICE), 1);
            assert_eq!(token.balance_of(&BOB), 1);
        })
    });
    result.assert_success();
    assert_eq!(transfers(&result, &addr), vec![(ALICE, BOB, id(1))]);
}

#[test]
fn approvals_and_operators_authorize_transfers() {
    let env = TestEnvironment::no_cost();
    let addr = deploy_token(&env);

    // Alice approves Bob for token 1 and makes Operator her operator.
    let result = env.transact(|tx| {
        tx.call(&ALICE).run(move || {
            let token = ERC721Client::new(&addr);
            token.approve(&BOB, &id(1));
            assert_eq!(token.get_approved(&id(1)), BOB);

            token.set_approval_for_all(&OPERATOR, true);
            assert!(token.is_approved_for_all(&ALICE, &OPERATOR));
            assert!(!token.is_approved_for_all(&ALICE, &BOB));
        })
    });
    result.assert_success();
    assert_eq!(approvals(&result, &addr), vec![(ALICE, BOB, id(1))]);
    assert_eq!(
        approvals_for_all(&result, &addr),
        vec![(ALICE, OPERATOR, true)]
    );
    env.advance_block();

    // A per-token approval moves that token and nothing else, and does not
    // let its holder delegate further.
    let result = env.transact(|tx| {
        tx.call(&BOB).run(move || {
            let token = ERC721Client::new(&addr);
            assert_eq!(
                token.try_approve(&NOBODY, &id(1)).unwrap_err().try_as(),
                Some(ERC721Error::InvalidApprover),
            );
            assert_eq!(
                token
                    .try_transfer_from(&ALICE, &BOB, &id(2))
                    .unwrap_err()
                    .try_as(),
                Some(ERC721Error::InsufficientApproval),
            );

            token.transfer_from(&ALICE, &BOB, &id(1));
            assert_eq!(token.owner_of(&id(1)), BOB);
            // The transfer consumed the approval.
            assert_eq!(token.get_approved(&id(1)), *CONTRACT_ADDR_ZERO);
        })
    });
    result.assert_success();
    env.advance_block();

    // An operator may approve and transfer any of the owner's tokens.
    let result = env.transact(|tx| {
        tx.call(&OPERATOR).run(move || {
            let token = ERC721Client::new(&addr);
            token.approve(&NOBODY, &id(2));
            assert_eq!(token.get_approved(&id(2)), NOBODY);
            token.approve(CONTRACT_ADDR_ZERO, &id(2));
            assert_eq!(token.get_approved(&id(2)), *CONTRACT_ADDR_ZERO);

            token.transfer_from(&ALICE, &OPERATOR, &id(2));
            assert_eq!(token.owner_of(&id(2)), OPERATOR);
            assert_eq!(token.balance_of(&ALICE), 0);
        })
    });
    result.assert_success();
    assert_eq!(
        approvals(&result, &addr),
        vec![(ALICE, NOBODY, id(2)), (ALICE, *CONTRACT_ADDR_ZERO, id(2))]
    );
    env.advance_block();

    // Revoking the operator takes effect immediately.
    let result = env.transact(|tx| {
        tx.call(&OPERATOR).run(move || {
            let token = ERC721Client::new(&addr);
            token.transfer_from(&OPERATOR, &ALICE, &id(2));
        })
    });
    result.assert_success();
    env.advance_block();
    let result = env.transact(|tx| {
        tx.call(&ALICE).run(move || {
            ERC721Client::new(&addr).set_approval_for_all(&OPERATOR, false);
        })
    });
    result.assert_success();
    assert_eq!(
        approvals_for_all(&result, &addr),
        vec![(ALICE, OPERATOR, false)]
    );
    env.advance_block();
    let result = env.transact(|tx| {
        tx.call(&OPERATOR).run(move || {
            let token = ERC721Client::new(&addr);
            assert!(!token.is_approved_for_all(&ALICE, &OPERATOR));
            assert_eq!(
                token
                    .try_transfer_from(&ALICE, &OPERATOR, &id(2))
                    .unwrap_err()
                    .try_as(),
                Some(ERC721Error::InsufficientApproval),
            );
        })
    });
    result.assert_success();
}

#[test]
fn safe_transfer_consults_the_receiver() {
    let env = TestEnvironment::no_cost();
    let addr = deploy_token(&env);
    let (receiver, counter) = deploy_destinations(&env);

    let result = env.transact(|tx| {
        tx.call(&ALICE).run(move || {
            let token = ERC721Client::new(&addr);
            let data = TransferData::from_slice(b"hello").unwrap();

            // A contract with the hook takes the token and sees the call.
            token.safe_transfer_from(&ALICE, &receiver, &id(1), data.clone());
            assert_eq!(token.owner_of(&id(1)), receiver);
            assert_eq!(
                ReceiverClient::new(&receiver).last_delivery(),
                Some(Delivery {
                    operator: ALICE,
                    from: ALICE,
                    token_id: id(1),
                    data,
                })
            );

            // A contract without the hook, or no contract at all, cannot
            // take custody, and the token stays put.
            let invalid_receiver = |r: Result<(), CallError>| {
                assert_eq!(
                    r.unwrap_err().try_as(),
                    Some(ERC721Error::InvalidReceiver)
                );
            };
            invalid_receiver(token.try_safe_transfer_from(
                &ALICE,
                &counter,
                &id(2),
                TransferData::new(),
            ));
            invalid_receiver(token.try_safe_transfer_from(
                &ALICE,
                &NOBODY,
                &id(2),
                TransferData::new(),
            ));
            assert_eq!(token.owner_of(&id(2)), ALICE);

            // A receiver that throws rejects the transfer with its own
            // reason, which reaches the caller undeclared but decodable.
            let refused = token
                .try_safe_transfer_from(
                    &ALICE,
                    &receiver,
                    &id(2),
                    TransferData::from_slice(REJECT_DATA).unwrap(),
                )
                .unwrap_err();
            assert_eq!(refused.try_as(), Some(ReceiverError::Refused));
            assert_eq!(token.owner_of(&id(2)), ALICE);
            assert_eq!(token.balance_of(&receiver), 1);

            // Plain `transfer_from` asks nobody.
            token.transfer_from(&ALICE, &counter, &id(2));
            assert_eq!(token.owner_of(&id(2)), counter);
        })
    });
    result.assert_success();
    assert_eq!(
        transfers(&result, &addr),
        vec![(ALICE, receiver, id(1)), (ALICE, counter, id(2))]
    );
}

#[test]
fn burn_needs_authorization_and_removes_the_token() {
    let env = TestEnvironment::no_cost();
    let addr = deploy_token(&env);

    let result = env.transact(|tx| {
        tx.call(&BOB).run(move || {
            assert_eq!(
                NftAdminClient::new(&addr)
                    .try_burn(&id(1))
                    .unwrap_err()
                    .try_as(),
                Some(ERC721Error::InsufficientApproval),
            );
        })
    });
    result.assert_success();
    env.advance_block();

    let result = env.transact(|tx| {
        tx.call(&ALICE).run(move || {
            let token = ERC721Client::new(&addr);
            token.approve(&BOB, &id(2));
            NftAdminClient::new(&addr).burn(&id(1));

            assert_eq!(
                token.try_owner_of(&id(1)).unwrap_err().try_as(),
                Some(ERC721Error::NonexistentToken),
            );
            assert_eq!(
                ERC721MetadataClient::new(&addr)
                    .try_token_metadata(&id(1))
                    .unwrap_err()
                    .try_as(),
                Some(ERC721Error::NonexistentToken),
            );
            assert_eq!(token.balance_of(&ALICE), 1);
        })
    });
    result.assert_success();
    assert_eq!(burns(&result, &addr), vec![(ALICE, id(1))]);
    assert!(transfers(&result, &addr).is_empty());
    env.advance_block();

    // The approved address may burn, and a burned id can be minted again.
    let result = env.transact(|tx| {
        tx.call(&BOB).run(move || {
            NftAdminClient::new(&addr).burn(&id(2));
            assert_eq!(ERC721Client::new(&addr).balance_of(&ALICE), 0);
        })
    });
    result.assert_success();
    env.advance_block();
    let result = env.transact(|tx| {
        tx.call(&ADMIN).run(move || {
            NftAdminClient::new(&addr).mint(&BOB, &id(1), uri_metadata(1));
            assert_eq!(ERC721Client::new(&addr).owner_of(&id(1)), BOB);
        })
    });
    result.assert_success();
}

/// Two transactions in one block both execute against the previous
/// block's state, in which Alice owns the token: each would record Bob as
/// the owner and move one balance. The exclusive lock on the owner slot
/// lets the first commit and fails the second, so the balances move once.
#[test]
fn two_transfers_of_one_token_in_a_block_commit_once() {
    let env = TestEnvironment::no_cost();
    let addr = deploy_token(&env);

    let transfer = || {
        env.transact(|tx| {
            tx.call(&ALICE).run(move || {
                ERC721Client::new(&addr).transfer_from(&ALICE, &BOB, &id(1));
            })
        })
    };
    transfer().assert_success();
    transfer().assert_commit_failed_nondeterministically();
    env.advance_block();

    let result = env.transact(|tx| {
        tx.call(&ALICE).run(move || {
            let token = ERC721Client::new(&addr);
            assert_eq!(token.owner_of(&id(1)), BOB);
            assert_eq!(token.balance_of(&ALICE), 1);
            assert_eq!(token.balance_of(&BOB), 1);
        })
    });
    result.assert_success();
}

#[test]
fn supports_interface_reports_the_token_interfaces() {
    let env = TestEnvironment::no_cost();
    let addr = deploy_token(&env);
    let (receiver, _) = deploy_destinations(&env);

    let result =
        env.transact(|tx| {
            tx.call(&ALICE).run(move || {
                let erc165 = ERC165Client::new(&addr);
                assert!(
                    erc165.supports_interface(ERC721Entrypoints::INTERFACE_ID)
                );
                assert!(erc165.supports_interface(
                    ERC721MetadataEntrypoints::INTERFACE_ID
                ));
                assert!(!erc165.supports_interface(
                    ERC721ReceiverEntrypoints::INTERFACE_ID
                ));

                // The receiver fixture has the hook but no ERC-165, so a probe
                // cannot confirm it; the safe transfer path calls the hook
                // directly rather than asking first.
                assert_eq!(
                    try_probe_interface(
                        &receiver,
                        ERC721ReceiverEntrypoints::INTERFACE_ID
                    ),
                    Ok(false)
                );
            })
        });
    result.assert_success();
}
