//! Runtime tests; transactions in one block share a snapshot.
use erc20_issuer::*;
use thruput_sdk::*;

const TOKEN: ContractAddress = ContractAddress::new([0x10; 32]);
const ADMIN: ContractAddress = ContractAddress::new([0x11; 32]);
const MINTER: ContractAddress = ContractAddress::new([0x12; 32]);
const ALICE: ContractAddress = ContractAddress::new([0x13; 32]);
const BOB: ContractAddress = ContractAddress::new([0x14; 32]);
const SPENDER: ContractAddress = ContractAddress::new([0x15; 32]);
const OTHER: ContractAddress = ContractAddress::new([0x16; 32]);
const NONCE: SHA256Hash = SHA256Hash::new([0x17; 32]);
fn token() -> ERC20Client {
    ERC20Client::new(&TOKEN)
}
fn admin() -> ERC20AdminClient {
    ERC20AdminClient::new(&TOKEN)
}
fn auth() -> ERC20AuthorizationClient {
    ERC20AuthorizationClient::new(&TOKEN)
}
fn burn() -> ERC20BurnClient {
    ERC20BurnClient::new(&TOKEN)
}
fn clawback() -> ERC20ClawbackClient {
    ERC20ClawbackClient::new(&TOKEN)
}
fn policy() -> Policy {
    Policy {
        auth_required: false,
        auth_revocable: true,
        clawback_enabled: true,
    }
}
fn call(
    env: &TestEnvironmentHandle,
    who: &ContractAddress,
    f: impl FnOnce() + std::panic::UnwindSafe,
) -> TransactResult {
    env.transact(|tx| tx.call(who).run(f))
}
fn register(
    env: &TestEnvironmentHandle,
) -> (thruput_sdk::types::Contract, SHA256Hash) {
    if let Some(path) = std::env::var_os("ISSUER_TOKEN_RV32_ARTIFACT") {
        let contract = thruput_sdk::types::Contract::kRv32 {
            rv32_contract: thruput_sdk::types::contract::ContractBlob {
                data: thruput_sdk::xdr::BytesM(
                    std::fs::read(path).expect("read RV32 artifact"),
                ),
            },
        };
        let hash = env.setup_create_contract(&contract);
        (contract, hash)
    } else {
        let mock = erc20_issuer::register_mock_contract(env);
        (mock.as_contract(), *mock.as_hash())
    }
}
fn setup_with(policy: Policy) -> TestEnvironmentHandle {
    let env = TestEnvironment::no_cost();
    let (contract, _) = register(&env);
    env.setup_create_deployment(&TOKEN, &contract);
    call(&env, &ADMIN, || {
        admin().initialize(
            &MINTER,
            "Reference Token".try_into().unwrap(),
            "ABCDEFGHIJKL".try_into().unwrap(),
            7,
            policy,
        )
    })
    .assert_success();
    env.advance_block();
    env
}
fn setup() -> TestEnvironmentHandle {
    setup_with(policy())
}
fn mint(env: &TestEnvironmentHandle, amount: Amount) {
    call(env, &MINTER, || token().mint(&ALICE, amount)).assert_success();
    env.advance_block();
}
fn adjust(env: &TestEnvironmentHandle, difference: i128) {
    call(env, &ALICE, || {
        token().adjust_allowance(&SPENDER, difference)
    })
    .assert_success();
    env.advance_block();
}
fn clear(env: &TestEnvironmentHandle) {
    call(env, &ALICE, || token().delete_allowance(&SPENDER)).assert_success();
    env.advance_block();
}
fn state(
    env: &TestEnvironmentHandle,
    alice: Amount,
    bob: Amount,
    allowance: Amount,
    supply: Amount,
) {
    call(env, &OTHER, || {
        assert_eq!(token().get_balance_of(&ALICE), alice);
        assert_eq!(token().get_balance_of(&BOB), bob);
        assert_eq!(token().allowance(&ALICE, &SPENDER), allowance);
        assert_eq!(token().total_supply(), supply);
    })
    .assert_success();
}
fn error(r: TransactResult, expected: ERC20ControlsError) {
    assert_eq!(r.assert_abort_as::<ERC20ControlsError>(), expected);
    r.assert_no_events();
}
fn erc_error(r: TransactResult, expected: ERC20Error) {
    assert_eq!(r.assert_abort_as::<ERC20Error>(), expected);
    r.assert_no_events();
}

#[test]
fn atomic_deployment_metadata_and_all_initializers_share_one_guard() {
    let env = TestEnvironment::no_cost();
    let (_, hash) = register(&env);
    let r = env.transact(|tx| {
        tx.call(&ADMIN).run(|| {
            let addr = deploy_contract(&NONCE, &hash);
            ERC20AdminClient::new(&addr).initialize(
                &MINTER,
                "Name".try_into().unwrap(),
                "ABCDEFGHIJKL".try_into().unwrap(),
                7,
                policy(),
            );
            let meta = ERC20MetadataClient::new(&addr);
            assert_eq!(meta.symbol().unwrap().as_str(), "ABCDEFGHIJKL");
            assert_eq!(meta.name().unwrap().as_str(), "Name");
            assert_eq!(meta.decimals(), Some(7));
            assert_eq!(ERC20AdminClient::new(&addr).admin(), ADMIN);
            assert_eq!(ERC20Client::new(&addr).total_supply(), 0);
            addr
        })
    });
    r.assert_success();
    let addr = r.assert_return_as::<ContractAddress>();
    env.advance_block();
    erc_error(
        call(&env, &OTHER, || ERC20Client::new(&addr).initialize(&OTHER)),
        ERC20Error::AlreadyInitialized,
    );
    erc_error(
        call(&env, &OTHER, || {
            ERC20MetadataClient::new(&addr)
                .initialize(&OTHER, None, None, None)
        }),
        ERC20Error::AlreadyInitialized,
    );
    erc_error(
        call(&env, &OTHER, || {
            ERC20AdminClient::new(&addr).initialize(
                &OTHER,
                "x".try_into().unwrap(),
                "X".try_into().unwrap(),
                0,
                policy(),
            )
        }),
        ERC20Error::AlreadyInitialized,
    );
    assert!(Symbol::try_from("ABCDEFGHIJKLM").is_err());
}

#[test]
fn invalid_policy_and_uninitialized_calls_leave_no_state() {
    let env = TestEnvironment::no_cost();
    let (contract, _) = register(&env);
    env.setup_create_deployment(&TOKEN, &contract);
    error(
        call(&env, &ALICE, || {
            token().get_balance();
        }),
        ERC20ControlsError::NotInitialized,
    );
    error(
        call(&env, &ADMIN, || {
            admin().initialize(
                &MINTER,
                "x".try_into().unwrap(),
                "X".try_into().unwrap(),
                7,
                Policy {
                    auth_required: false,
                    auth_revocable: false,
                    clawback_enabled: true,
                },
            )
        }),
        ERC20ControlsError::InvalidPolicy,
    );
    call(&env, &ADMIN, || token().initialize(&ADMIN)).assert_success();
}

#[test]
fn adjustment_partial_spends_clear_and_standard_events() {
    let env = setup();
    mint(&env, 100);
    let r = call(&env, &ALICE, || token().adjust_allowance(&SPENDER, 60));
    r.assert_success();
    r.assert_event_count(1);
    r.assert_raw_event(0, |e| {
        let a = AllowanceChangeEvent::try_from(EmittedEvent::from(e.raw()))
            .unwrap();
        assert_eq!(a.owner, &ALICE);
        assert_eq!(a.spender, &SPENDER);
        assert_eq!(a.difference, 60);
    });
    env.advance_block();
    for _ in 0..2 {
        call(&env, &SPENDER, || token().transfer_from(&ALICE, &BOB, 10))
            .assert_success();
    }
    env.advance_block();
    state(&env, 80, 20, 40, 100);
    adjust(&env, -33);
    state(&env, 80, 20, 7, 100);
    clear(&env);
    erc_error(
        call(&env, &SPENDER, || token().transfer_from(&ALICE, &BOB, 1)),
        ERC20Error::TooLarge,
    );
    state(&env, 80, 20, 0, 100);
}

#[test]
fn frozen_holders_cannot_move_mint_or_burn_but_can_revoke() {
    let env = setup();
    mint(&env, 100);
    adjust(&env, 50);
    call(&env, &ADMIN, || auth().set_authorized(&ALICE, false))
        .assert_success();
    env.advance_block();
    for r in [
        call(&env, &ALICE, || token().transfer(&BOB, 1)),
        call(&env, &BOB, || token().transfer(&ALICE, 0)),
        call(&env, &SPENDER, || token().transfer_from(&ALICE, &BOB, 1)),
        call(&env, &ALICE, || burn().burn(1)),
        call(&env, &SPENDER, || burn().burn_from(&ALICE, 1)),
        call(&env, &MINTER, || token().mint(&ALICE, 1)),
    ] {
        error(r, ERC20ControlsError::HolderNotAuthorized);
    }
    clear(&env);
    let r = call(&env, &ADMIN, || clawback().clawback(&ALICE, 10));
    r.assert_success();
    r.assert_event_count(2);
    r.assert_raw_event(0, |e| {
        assert_eq!(
            BurnEvent::try_from(EmittedEvent::from(e.raw()))
                .unwrap()
                .amount,
            10
        );
    });
    r.assert_raw_event(1, |e| {
        assert_eq!(e.topic(0), Some(ClawbackEvent::SIGNATURE))
    });
    env.advance_block();
    state(&env, 90, 0, 0, 90);
    call(&env, &ADMIN, || auth().set_authorized(&ALICE, true))
        .assert_success();
    env.advance_block();
    call(&env, &ALICE, || token().transfer(&BOB, 10)).assert_success();
    env.advance_block();
    state(&env, 80, 10, 0, 90);
}

#[test]
fn required_authorization_checks_both_holders_and_rolls_back_allowance() {
    let env = setup_with(Policy {
        auth_required: true,
        ..policy()
    });
    error(
        call(&env, &MINTER, || token().mint(&ALICE, 10)),
        ERC20ControlsError::HolderNotAuthorized,
    );
    call(&env, &ADMIN, || auth().set_authorized(&ALICE, true))
        .assert_success();
    env.advance_block();
    mint(&env, 10);
    adjust(&env, 10);
    error(
        call(&env, &SPENDER, || token().transfer_from(&ALICE, &BOB, 5)),
        ERC20ControlsError::HolderNotAuthorized,
    );
    state(&env, 10, 0, 10, 10);
    call(&env, &ADMIN, || auth().set_authorized(&BOB, true)).assert_success();
    env.advance_block();
    call(&env, &SPENDER, || token().transfer_from(&ALICE, &BOB, 5))
        .assert_success();
    env.advance_block();
    state(&env, 5, 5, 5, 10);
}

#[test]
fn disabled_clawback_blocks_forced_destruction() {
    let env = setup_with(Policy {
        auth_required: false,
        auth_revocable: false,
        clawback_enabled: false,
    });
    mint(&env, 10);
    for r in [
        call(&env, &ADMIN, || auth().set_authorized(&ALICE, false)),
        call(&env, &ADMIN, || clawback().clawback(&ALICE, 1)),
    ] {
        error(r, ERC20ControlsError::OperationDisabled);
    }
    call(&env, &ALICE, || burn().burn(1)).assert_success();
    env.advance_block();
    state(&env, 9, 0, 0, 9);
}

#[test]
fn roles_are_separate_and_rotation_takes_effect_next_block() {
    let env = setup();
    erc_error(
        call(&env, &ADMIN, || token().mint(&ALICE, 1)),
        ERC20Error::MustBeContractOwner,
    );
    erc_error(
        call(&env, &MINTER, || clawback().clawback(&ALICE, 0)),
        ERC20Error::MustBeContractOwner,
    );
    for r in [
        call(&env, &MINTER, || auth().set_authorized(&ALICE, false)),
        call(&env, &MINTER, || admin().set_admin(&MINTER)),
        call(&env, &MINTER, || admin().set_minter(&MINTER)),
    ] {
        error(r, ERC20ControlsError::Unauthorized);
    }
    call(&env, &ADMIN, || {
        admin().set_minter(&OTHER);
        admin().set_admin(&BOB);
    })
    .assert_success();
    // Snapshot visibility: old minter remains valid in this block.
    call(&env, &MINTER, || token().mint(&ALICE, 5)).assert_success();
    env.advance_block();
    erc_error(
        call(&env, &MINTER, || token().mint(&ALICE, 1)),
        ERC20Error::MustBeContractOwner,
    );
    error(
        call(&env, &ADMIN, || admin().set_minter(&MINTER)),
        ERC20ControlsError::Unauthorized,
    );
    call(&env, &OTHER, || token().mint(&ALICE, 5)).assert_success();
    env.advance_block();
    state(&env, 10, 0, 0, 10);
}

#[test]
fn range_supply_and_caught_failure_preserve_balances_and_allowance() {
    let env = setup();
    mint(&env, i128::MAX);
    erc_error(
        call(&env, &MINTER, || token().mint(&BOB, 1)),
        ERC20Error::TooLarge,
    );
    adjust(&env, i128::MAX);
    erc_error(
        call(&env, &ALICE, || token().adjust_allowance(&SPENDER, 1)),
        ERC20Error::TooLarge,
    );
    call(&env, &SPENDER, || {
        token().transfer_from(&ALICE, &BOB, i128::MAX)
    })
    .assert_success();
    env.advance_block();
    call(&env, &BOB, || burn().burn(i128::MAX)).assert_success();
    env.advance_block();
    state(&env, 0, 0, 0, 0);
    adjust(&env, 100);
    call(&env, &SPENDER, || {
        assert_eq!(
            token()
                .try_transfer_from(&ALICE, &BOB, 1)
                .unwrap_err()
                .try_as(),
            Some(ERC20Error::TooLarge)
        );
        assert_eq!(token().allowance(&ALICE, &SPENDER), 100);
    })
    .assert_success();
    state(&env, 0, 0, 100, 0);
}

#[test]
fn absolute_set_or_clearing_conflicts_with_same_block_spending() {
    for difference in [None, Some(-10), Some(0), Some(10)] {
        for adjustment_first in [false, true] {
            let env = setup();
            mint(&env, 100);
            adjust(&env, 50);
            let grant = || {
                call(&env, &ALICE, || {
                    token().delete_allowance(&SPENDER);
                    if let Some(difference) = difference {
                        token().adjust_allowance(&SPENDER, 50 + difference);
                    }
                })
            };
            let spend = || {
                call(&env, &SPENDER, || {
                    token().transfer_from(&ALICE, &BOB, 10)
                })
            };
            let (first, second) = if adjustment_first {
                (grant(), spend())
            } else {
                (spend(), grant())
            };
            first.assert_success();
            second.assert_commit_failed_nondeterministically();
            env.advance_block();
            if adjustment_first {
                state(&env, 100, 0, difference.map_or(0, |d| 50 + d), 100);
            } else {
                state(&env, 90, 10, 40, 100);
            }
        }
    }
}

#[test]
fn supply_and_holder_limits_apply_to_combined_transactions() {
    let env = setup();
    mint(&env, i128::MAX - 10);
    call(&env, &MINTER, || token().mint(&ALICE, 6)).assert_success();
    call(&env, &MINTER, || token().mint(&BOB, 6))
        .assert_commit_failed_nondeterministically();
    env.advance_block();
    state(&env, i128::MAX - 4, 0, 0, i128::MAX - 4);
    let env = setup();
    mint(&env, 10);
    call(&env, &ALICE, || token().transfer(&BOB, 6)).assert_success();
    call(&env, &ALICE, || token().transfer(&OTHER, 6))
        .assert_commit_failed_nondeterministically();
    env.advance_block();
    state(&env, 4, 6, 0, 10);
}

#[test]
fn independent_pairs_and_conflicting_absolute_sets() {
    let env = setup();
    let set = |owner: ContractAddress, spender: ContractAddress| {
        call(&env, &owner, move || {
            token().delete_allowance(&spender);
            token().adjust_allowance(&spender, 10);
        })
    };
    set(ALICE, SPENDER).assert_success();
    set(ALICE, OTHER).assert_success();
    set(BOB, SPENDER).assert_success();
    set(ALICE, SPENDER).assert_commit_failed_nondeterministically();
}

#[test]
fn zero_self_transfer_and_delegated_burn_use_standard_events() {
    let env = setup();
    mint(&env, 10);
    adjust(&env, 5);
    call(&env, &ALICE, || {
        token().transfer(&ALICE, 10);
        token().transfer(&BOB, 0);
    })
    .assert_success();
    env.advance_block();
    let r = call(&env, &SPENDER, || burn().burn_from(&ALICE, 5));
    r.assert_success();
    r.assert_event_count(1);
    r.assert_raw_event(0, |e| {
        assert_eq!(
            BurnEvent::try_from(EmittedEvent::from(e.raw()))
                .unwrap()
                .amount,
            5
        )
    });
    env.advance_block();
    state(&env, 5, 0, 0, 5);
}

#[test]
fn signed_transfer_checks_holder_policy_and_leaves_no_allowance() {
    use thruput_sdk::mintable::{AuthTokenPreimage, MintableType};
    use thruput_sdk::signer::mint_auth_token;
    type GrantData = <PermitGrant as MintableType>::Preimage;
    let env = setup();
    mint(&env, 20);
    let send = || {
        call(&env, &ALICE, || {
            let grant = mint_auth_token::<PermitGrant>(
                thruput_sdk::block::get_block_height(),
                AuthTokenPreimage {
                    consumer: TOKEN,
                    data: GrantData {
                        spender: ALICE,
                        amount: 5,
                    },
                },
            );
            ERC20PermitClient::new(&TOKEN)
                .transfer_from_signed(grant, &BOB, 5);
            assert_eq!(token().allowance(&ALICE, &ALICE), 0);
        })
    };
    call(&env, &ADMIN, || auth().set_authorized(&BOB, false)).assert_success();
    env.advance_block();
    error(send(), ERC20ControlsError::HolderNotAuthorized);
    state(&env, 20, 0, 0, 20);
    call(&env, &ADMIN, || auth().set_authorized(&BOB, true)).assert_success();
    env.advance_block();
    send().assert_success();
    env.advance_block();
    state(&env, 15, 5, 0, 20);
}

#[test]
#[ignore = "exports runtime event bytes for downstream HTTP tests"]
fn export_cli_event_fixture() {
    let output = std::env::var_os("ISSUER_TOKEN_CLI_EVENTS_OUT")
        .expect("set ISSUER_TOKEN_CLI_EVENTS_OUT");
    let env = setup();
    type Operation = (&'static str, &'static ContractAddress, fn());
    let operations: [Operation; 8] = [
        ("mint", &MINTER, || token().mint(&ALICE, 100)),
        ("transfer", &ALICE, || token().transfer(&BOB, 25)),
        ("adjust_allowance", &ALICE, || {
            token().adjust_allowance(&SPENDER, 30)
        }),
        ("burn", &ALICE, || burn().burn(5)),
        ("clawback", &ADMIN, || clawback().clawback(&ALICE, 7)),
        ("set_authorized", &ADMIN, || {
            auth().set_authorized(&ALICE, false)
        }),
        ("set_minter", &ADMIN, || admin().set_minter(&OTHER)),
        ("set_admin", &ADMIN, || admin().set_admin(&OTHER)),
    ];
    let mut records = Vec::new();
    for (operation, caller, invoke) in operations {
        let result = call(&env, caller, invoke);
        result.assert_success();
        for raw in result.events() {
            let event = EmittedEvent::from(raw);
            let topics: Vec<_> = (0..event.num_topics())
                .map(|i| {
                    format!(
                        "\"0x{}\"",
                        thruput_sdk::hex_encode(event.topic(i).unwrap())
                    )
                })
                .collect();
            records.push(format!("  {{\"operation\":\"{operation}\",\"event\":{{\"contractAddress\":\"0x{}\",\"topics\":[{}],\"data\":\"0x{}\"}}}}", thruput_sdk::hex_encode(&event.raw().contract_address), topics.join(","), thruput_sdk::hex_encode(&event.payload().unwrap())));
        }
        env.advance_block();
    }
    std::fs::write(output, format!("[\n{}\n]\n", records.join(",\n")))
        .unwrap();
}

#[test]
fn clearing_a_frozen_zero_balance_preserves_authorization_metadata() {
    let env = setup();
    mint(&env, 10);
    call(&env, &ADMIN, || auth().set_authorized(&ALICE, false))
        .assert_success();
    env.advance_block();
    erc_error(
        call(&env, &ALICE, || token().delete_balance()),
        ERC20Error::BalanceNotZero,
    );
    call(&env, &ADMIN, || clawback().clawback(&ALICE, 10)).assert_success();
    env.advance_block();
    let cleared = call(&env, &ALICE, || token().delete_balance());
    cleared.assert_success();
    cleared.assert_no_events();
    env.advance_block();
    call(&env, &ALICE, || {
        assert_eq!(token().get_balance(), 0);
        assert!(!auth().authorized(&ALICE));
        let metadata = ERC20MetadataClient::new(&TOKEN);
        assert_eq!(metadata.name().unwrap().as_str(), "Reference Token");
        assert_eq!(metadata.symbol().unwrap().as_str(), "ABCDEFGHIJKL");
        assert_eq!(metadata.decimals(), Some(7));
        token().delete_balance();
    })
    .assert_success();
    error(
        call(&env, &MINTER, || token().mint(&ALICE, 1)),
        ERC20ControlsError::HolderNotAuthorized,
    );
}
