//! Host-side tests for the pausable ERC20 example: metadata, pause
//! gating, and role-based access control across multiple callers.

use erc20_pausable::PAUSER_ROLE;
use groundhog_access_control_if::{
    AccessControlClient, AccessControlError, DEFAULT_ADMIN_ROLE, Role,
    RoleGrantedEvent, RoleRevokedEvent,
};
use groundhog_erc20_if::{ERC20Client, ERC20Error};
use groundhog_erc20_metadata_if::ERC20MetadataClient;
use groundhog_erc20_pausable_if::{
    ERC20PausableClient, ERC20PausableError, PausedEvent, UnpausedEvent,
};
use groundhog_erc165_if::{ERC165Client, InterfaceIds, InterfaceSupport};
use sdk::call::CallError;
use sdk::*;

hash_ident! { static TEST_NONCE: SHA256Hash; }
hash_ident! { static ADMIN: ContractAddress; }
hash_ident! { static OTHER: ContractAddress; }

const TOKEN_NAME: &str = "Pausable Token";
const TOKEN_SYMBOL: &str = "PAUSE";
const TOKEN_DECIMALS: u32 = 6;

/// Deploy the contract and initialize it (with metadata) as `ADMIN`.
fn deploy_and_init(env: &TestEnvironmentHandle) -> ContractAddress {
    let contract = erc20_pausable::register_mock_contract(env);
    let hash = *contract.as_hash();

    let result = env.transact(|tx| {
        tx.call(&ADMIN).run(move || {
            let addr = deploy_contract(&TEST_NONCE, &hash);
            ERC20MetadataClient::new(&addr).initialize(
                &ADMIN,
                Some(TOKEN_NAME.try_into().unwrap()),
                Some(TOKEN_SYMBOL.try_into().unwrap()),
                Some(TOKEN_DECIMALS),
            );
            addr
        })
    });
    result.assert_success();
    env.advance_block();
    result.assert_return_as()
}

fn granted_events(
    result: &TransactResult,
) -> Vec<(Role, ContractAddress, ContractAddress)> {
    result
        .events()
        .iter()
        .filter_map(|e| RoleGrantedEvent::try_from(EmittedEvent::from(e)).ok())
        .map(|e| (*e.role, *e.account, e.sender))
        .collect()
}

fn revoked_events(
    result: &TransactResult,
) -> Vec<(Role, ContractAddress, ContractAddress)> {
    result
        .events()
        .iter()
        .filter_map(|e| RoleRevokedEvent::try_from(EmittedEvent::from(e)).ok())
        .map(|e| (*e.role, *e.account, e.sender))
        .collect()
}

#[test]
fn initialize_sets_metadata_and_roles() {
    let env = TestEnvironment::no_cost();
    let contract = erc20_pausable::register_mock_contract(&env);
    let hash = *contract.as_hash();

    let result = env.transact(|tx| {
        tx.call(&ADMIN).run(move || {
            let addr = deploy_contract(&TEST_NONCE, &hash);

            let meta = ERC20MetadataClient::new(&addr);
            meta.initialize(
                &ADMIN,
                Some(TOKEN_NAME.try_into().unwrap()),
                Some(TOKEN_SYMBOL.try_into().unwrap()),
                Some(TOKEN_DECIMALS),
            );
            assert_eq!(
                Some(TOKEN_NAME),
                meta.name().as_ref().map(|n| n.as_str())
            );
            assert_eq!(
                Some(TOKEN_SYMBOL),
                meta.symbol().as_ref().map(|s| s.as_str())
            );
            assert_eq!(Some(TOKEN_DECIMALS), meta.decimals());

            // A second initialize (through either interface) is rejected.
            assert_eq!(
                ERC20Client::new(&addr)
                    .try_initialize(&ADMIN)
                    .unwrap_err()
                    .try_as(),
                Some(ERC20Error::AlreadyInitialized),
            );

            // The administrator starts with both roles; nobody else has any.
            let access = AccessControlClient::new(&addr);
            assert!(access.has_role(DEFAULT_ADMIN_ROLE, &ADMIN));
            assert!(access.has_role(&PAUSER_ROLE, &ADMIN));
            assert!(!access.has_role(DEFAULT_ADMIN_ROLE, &OTHER));
            assert!(!access.has_role(&PAUSER_ROLE, &OTHER));
            assert_eq!(
                access.get_role_admin(&PAUSER_ROLE),
                *DEFAULT_ADMIN_ROLE
            );

            // The token starts unpaused.
            assert!(!ERC20PausableClient::new(&addr).paused());
        })
    });
    result.assert_success();

    // Initialization granted exactly the two administrator roles.
    assert_eq!(
        granted_events(&result),
        vec![
            (*DEFAULT_ADMIN_ROLE, ADMIN, ADMIN),
            (PAUSER_ROLE, ADMIN, ADMIN),
        ]
    );
}

#[test]
fn pause_blocks_balance_changes() {
    let env = TestEnvironment::no_cost();
    let addr = deploy_and_init(&env);

    // Fund both parties while unpaused.
    let result = env.transact(|tx| {
        tx.call(&ADMIN).run(move || {
            let token = ERC20Client::new(&addr);
            token.mint(&ADMIN, 100);
            token.transfer(&OTHER, 30);
        })
    });
    result.assert_success();
    env.advance_block();

    // A non-pauser cannot pause, but can still transfer.
    let result = env.transact(|tx| {
        tx.call(&OTHER).run(move || {
            assert_eq!(
                ERC20PausableClient::new(&addr)
                    .try_pause()
                    .unwrap_err()
                    .try_as(),
                Some(ERC20PausableError::MustBePauser),
            );
            ERC20Client::new(&addr).transfer(&ADMIN, 5);
        })
    });
    result.assert_success();
    env.advance_block();

    // Pause, then verify every balance-changing operation is rejected
    // while reads and allowance adjustments stay available.
    let result = env.transact(|tx| {
        tx.call(&ADMIN).run(move || {
            let pausable = ERC20PausableClient::new(&addr);
            pausable.pause();
            assert!(pausable.paused());

            let token = ERC20Client::new(&addr);
            // `EnforcedPause` is deliberately absent from ERC20's `throws`
            // contract, yet it is reached here exactly like a declared
            // error: the selector identifies the type, not the interface.
            let enforced_pause = |r: Result<(), CallError>| {
                assert_eq!(
                    r.unwrap_err().try_as(),
                    Some(ERC20PausableError::EnforcedPause),
                );
            };
            enforced_pause(token.try_transfer(&OTHER, 1));
            enforced_pause(token.try_transfer_from(&OTHER, &ADMIN, 1));
            enforced_pause(token.try_mint(&ADMIN, 1));

            // Reads and allowance adjustments are not paused.
            assert_eq!(token.get_balance(), 75);
            assert_eq!(token.get_balance_of(&OTHER), 25);
            assert_eq!(token.total_supply(), 100);
            token.adjust_allowance(&OTHER, 10);
            assert_eq!(token.allowance(&ADMIN, &OTHER), 10);

            // Pausing an already-paused token is rejected.
            assert_eq!(
                pausable.try_pause().unwrap_err().try_as(),
                Some(ERC20PausableError::EnforcedPause),
            );
        })
    });
    result.assert_success();
    let paused: Vec<PausedEvent> = result
        .events()
        .iter()
        .filter_map(|e| PausedEvent::try_from(EmittedEvent::from(e)).ok())
        .collect();
    assert_eq!(paused, vec![PausedEvent::new(ADMIN)]);
    env.advance_block();

    // Unpause restores transfers; unpausing twice is rejected.
    let result = env.transact(|tx| {
        tx.call(&ADMIN).run(move || {
            let pausable = ERC20PausableClient::new(&addr);
            pausable.unpause();
            assert!(!pausable.paused());
            assert_eq!(
                pausable.try_unpause().unwrap_err().try_as(),
                Some(ERC20PausableError::ExpectedPause),
            );

            let token = ERC20Client::new(&addr);
            token.transfer(&OTHER, 5);
            assert_eq!(token.get_balance(), 70);
        })
    });
    result.assert_success();
    let unpaused: Vec<UnpausedEvent> = result
        .events()
        .iter()
        .filter_map(|e| UnpausedEvent::try_from(EmittedEvent::from(e)).ok())
        .collect();
    assert_eq!(unpaused, vec![UnpausedEvent::new(ADMIN)]);
}

#[test]
fn roles_gate_pausing_and_are_administered() {
    let env = TestEnvironment::no_cost();
    let addr = deploy_and_init(&env);

    // The admin grants OTHER the pauser role; regranting is a no-op.
    let result = env.transact(|tx| {
        tx.call(&ADMIN).run(move || {
            let access = AccessControlClient::new(&addr);
            access.grant_role(&PAUSER_ROLE, &OTHER);
            assert!(access.has_role(&PAUSER_ROLE, &OTHER));
            access.grant_role(&PAUSER_ROLE, &OTHER);
        })
    });
    result.assert_success();
    assert_eq!(granted_events(&result), vec![(PAUSER_ROLE, OTHER, ADMIN)]);
    env.advance_block();

    // OTHER can now pause and unpause, but still cannot administer
    // roles, and can renounce its own.
    let result = env.transact(|tx| {
        tx.call(&OTHER).run(move || {
            let pausable = ERC20PausableClient::new(&addr);
            pausable.pause();
            pausable.unpause();

            let access = AccessControlClient::new(&addr);
            assert_eq!(
                access
                    .try_grant_role(&PAUSER_ROLE, &OTHER)
                    .unwrap_err()
                    .try_as(),
                Some(AccessControlError::MissingRole),
            );
            assert_eq!(
                access
                    .try_revoke_role(&PAUSER_ROLE, &ADMIN)
                    .unwrap_err()
                    .try_as(),
                Some(AccessControlError::MissingRole),
            );

            access.renounce_role(&PAUSER_ROLE);
            assert!(!access.has_role(&PAUSER_ROLE, &OTHER));
            assert_eq!(
                pausable.try_pause().unwrap_err().try_as(),
                Some(ERC20PausableError::MustBePauser),
            );

            // Renouncing a role that isn't held is a no-op.
            access.renounce_role(&PAUSER_ROLE);
        })
    });
    result.assert_success();
    assert_eq!(revoked_events(&result), vec![(PAUSER_ROLE, OTHER, OTHER)]);
    env.advance_block();

    // The admin revokes a re-granted role; revoking an unheld role is a
    // no-op.
    let result = env.transact(|tx| {
        tx.call(&ADMIN).run(move || {
            let access = AccessControlClient::new(&addr);
            access.grant_role(&PAUSER_ROLE, &OTHER);
            access.revoke_role(&PAUSER_ROLE, &OTHER);
            assert!(!access.has_role(&PAUSER_ROLE, &OTHER));
            access.revoke_role(&PAUSER_ROLE, &OTHER);
        })
    });
    result.assert_success();
    assert_eq!(revoked_events(&result), vec![(PAUSER_ROLE, OTHER, ADMIN)]);
}

/// ERC-165: `supports_interface` answers for exactly the interfaces this
/// contract implements (ERC-165 itself included), and `implements` reads a
/// target without the entrypoint, or no contract at all, as `false`.
#[test]
fn supports_interface_reports_derived_claims() {
    use erc20_pausable::{
        AccessControl, ERC20, ERC20Metadata, ERC20Pausable,
        PausableTokenContract as Token,
    };

    hash_ident! { static BARE_NONCE: SHA256Hash; }

    let env = TestEnvironment::no_cost();
    let addr = deploy_and_init(&env);
    // A real contract that does not implement ERC-165.
    let bare_hash = *counter::register_mock_contract(&env).as_hash();

    let result = env.transact(|tx| {
        tx.call(&OTHER).run(move || {
            let bare_addr = deploy_contract(&BARE_NONCE, &bare_hash);
            let erc165 = ERC165Client::new(&addr);

            // Every implemented interface, named through the trait...
            for id in [
                groundhog_erc165_if::ERC165Entrypoints::INTERFACE_ID,
                <Token as ERC20>::INTERFACE_ID,
                <Token as ERC20Metadata>::INTERFACE_ID,
                <Token as ERC20Pausable>::INTERFACE_ID,
                <Token as AccessControl>::INTERFACE_ID,
            ] {
                assert!(erc165.supports_interface(id));
            }
            // ...and the interface crates agree on those IDs without
            // naming the implementing type.
            assert!(erc165.supports_interface(
                groundhog_erc20_if::ERC20Entrypoints::INTERFACE_ID
            ));
            // `implements` names the interface by its client.
            assert!(erc165.implements::<groundhog_erc20_if::ERC20Client>());
            assert!(erc165.implements::<ERC165Client>());

            // An interface it does not implement, and an arbitrary ID.
            assert!(!erc165.supports_interface(
                groundhog_signed_op_if::SignedOpEntrypoints::INTERFACE_ID
            ));
            assert!(!erc165.supports_interface([0xff; 8]));

            // A target without ERC-165, and no contract at all, are "no".
            assert!(
                !ERC165Client::new(&bare_addr)
                    .implements::<groundhog_erc20_if::ERC20Client>()
            );
            const EMPTY_ADDR: ContractAddress =
                ContractAddress::new([0x78; 32]);
            assert!(
                !ERC165Client::new(&EMPTY_ADDR)
                    .implements::<groundhog_erc20_if::ERC20Client>()
            );
        })
    });
    result.assert_success();
}

/// ERC-165 as a list: `supports_interfaces` answers every queried id in
/// query order (duplicates and unknown ids included) and agrees with
/// `supports_interface` id by id.
#[test]
fn supports_interfaces_answers_each_queried_id_in_order() {
    use erc20_pausable::{
        AccessControl, ERC20, ERC20Metadata, ERC20Pausable,
        PausableTokenContract as Token,
    };

    let env = TestEnvironment::no_cost();
    let addr = deploy_and_init(&env);

    let result = env.transact(|tx| {
        tx.call(&OTHER).run(move || {
            let erc165 = ERC165Client::new(&addr);

            let query = InterfaceIds::from_slice(&[
                <Token as ERC20>::INTERFACE_ID,
                groundhog_signed_op_if::SignedOpEntrypoints::INTERFACE_ID,
                <Token as ERC20Metadata>::INTERFACE_ID,
                [0xff; 8],
                <Token as ERC20Pausable>::INTERFACE_ID,
                <Token as AccessControl>::INTERFACE_ID,
                groundhog_erc165_if::ERC165Entrypoints::INTERFACE_ID,
                // A repeated id is answered again.
                <Token as ERC20>::INTERFACE_ID,
            ])
            .unwrap();
            let expected = InterfaceSupport::from_slice(&[
                true, false, true, false, true, true, true, true,
            ])
            .unwrap();

            // The list answer is the single answers, in query order.
            assert_eq!(erc165.supports_interfaces(&query), expected);
            for (id, supported) in query.iter().zip(&expected) {
                assert_eq!(erc165.supports_interface(*id), *supported);
            }

            // An empty query gets an empty answer.
            assert_eq!(
                erc165.supports_interfaces(&InterfaceIds::new()),
                InterfaceSupport::new()
            );
        })
    });
    result.assert_success();
}
