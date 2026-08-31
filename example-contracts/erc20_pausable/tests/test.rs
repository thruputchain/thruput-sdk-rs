//! Host-side tests for the pausable ERC20 example: metadata, pause
//! gating, and role-based access control across multiple callers.

use erc20_pausable::PAUSER_ROLE;
use groundhog_access_control_if::{
    AccessControlClient, AccessControlClientError, AccessControlError,
    DEFAULT_ADMIN_ROLE, Role, RoleGrantedEvent, RoleRevokedEvent,
};
use groundhog_erc20_if::{ERC20Client, ERC20ClientError, ERC20Error};
use groundhog_erc20_metadata_if::ERC20MetadataClient;
use groundhog_erc20_pausable_if::{
    ERC20PausableClient, ERC20PausableClientError, ERC20PausableError,
    PausedEvent, UnpausedEvent,
};
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
            assert!(matches!(
                ERC20Client::new(&addr).try_initialize(&ADMIN),
                Err(ERC20ClientError::ERC20Error(
                    ERC20Error::AlreadyInitialized
                ))
            ));

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
            assert!(matches!(
                ERC20PausableClient::new(&addr).try_pause(),
                Err(ERC20PausableClientError::ERC20PausableError(
                    ERC20PausableError::MustBePauser
                ))
            ));
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
            let enforced_pause = |r: Result<(), ERC20ClientError>| match r {
                Err(ERC20ClientError::Unknown(raw)) => {
                    assert_eq!(
                        raw.try_as::<ERC20PausableError>(),
                        Some(ERC20PausableError::EnforcedPause)
                    );
                }
                other => panic!("expected EnforcedPause, got {other:?}"),
            };
            enforced_pause(token.try_transfer(&OTHER, 1));
            enforced_pause(token.try_transfer_from(&OTHER, &ADMIN, 1));
            enforced_pause(token.try_mint(&ADMIN, 1));
            enforced_pause(token.try_burn(&ADMIN, 1));

            // Reads and allowance adjustments are not paused.
            assert_eq!(token.get_balance(), 75);
            assert_eq!(token.get_balance_of(&OTHER), 25);
            assert_eq!(token.total_supply(), 100);
            token.adjust_allowance(&OTHER, 10);
            assert_eq!(token.allowance(&ADMIN, &OTHER), 10);

            // Pausing an already-paused token is rejected.
            assert!(matches!(
                pausable.try_pause(),
                Err(ERC20PausableClientError::ERC20PausableError(
                    ERC20PausableError::EnforcedPause
                ))
            ));
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
            assert!(matches!(
                pausable.try_unpause(),
                Err(ERC20PausableClientError::ERC20PausableError(
                    ERC20PausableError::ExpectedPause
                ))
            ));

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
            assert!(matches!(
                access.try_grant_role(&PAUSER_ROLE, &OTHER),
                Err(AccessControlClientError::AccessControlError(
                    AccessControlError::MissingRole
                ))
            ));
            assert!(matches!(
                access.try_revoke_role(&PAUSER_ROLE, &ADMIN),
                Err(AccessControlClientError::AccessControlError(
                    AccessControlError::MissingRole
                ))
            ));

            access.renounce_role(&PAUSER_ROLE);
            assert!(!access.has_role(&PAUSER_ROLE, &OTHER));
            assert!(matches!(
                pausable.try_pause(),
                Err(ERC20PausableClientError::ERC20PausableError(
                    ERC20PausableError::MustBePauser
                ))
            ));

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
