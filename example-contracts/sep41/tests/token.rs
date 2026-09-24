//! Native contract/runtime tests. Transactions in one block share a snapshot;
//! advance the block before reading their committed effects.
use sdk::*;
use sep41::*;

const TOKEN: ContractAddress = ContractAddress::new([0x10; 32]);
const ADMIN: ContractAddress = ContractAddress::new([0x11; 32]);
const MINTER: ContractAddress = ContractAddress::new([0x12; 32]);
const ALICE: ContractAddress = ContractAddress::new([0x13; 32]);
const BOB: ContractAddress = ContractAddress::new([0x14; 32]);
const SPENDER: ContractAddress = ContractAddress::new([0x15; 32]);
const OTHER: ContractAddress = ContractAddress::new([0x16; 32]);
const NONCE: SHA256Hash = SHA256Hash::new([0x17; 32]);

fn token() -> TokenClient {
    TokenClient::new(&TOKEN)
}
fn admin() -> TokenAdminClient {
    TokenAdminClient::new(&TOKEN)
}
fn open_policy() -> Policy {
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
fn init() {
    admin().initialize(
        &MINTER,
        "Reference Token".try_into().unwrap(),
        "REF".try_into().unwrap(),
        7,
        open_policy(),
    );
}
// Set SEP41_RV32_ARTIFACT to run the same cases against a built guest image.
// An explicit bad path fails; the guest run must never fall back to a mock.
fn register(
    env: &TestEnvironmentHandle,
) -> (sdk::types::Contract, SHA256Hash) {
    if let Some(path) = std::env::var_os("SEP41_RV32_ARTIFACT") {
        let contract = sdk::types::Contract::kRv32 {
            rv32_contract: sdk::types::contract::ContractBlob {
                data: sdk::xdr::BytesM(
                    std::fs::read(path).expect("read RV32 artifact"),
                ),
            },
        };
        let hash = env.setup_create_contract(&contract);
        (contract, hash)
    } else {
        let mock = sep41::register_mock_contract(env);
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
            "REF".try_into().unwrap(),
            7,
            policy,
        );
    })
    .assert_success();
    env.advance_block();
    env
}
fn setup() -> TestEnvironmentHandle {
    setup_with(open_policy())
}
fn mint(env: &TestEnvironmentHandle, to: &ContractAddress, amount: i128) {
    call(env, &MINTER, || admin().mint(to, amount)).assert_success();
    env.advance_block();
}
fn approve(env: &TestEnvironmentHandle, amount: i128, expiry: u32) {
    call(env, &ALICE, || {
        token().approve(&ALICE, &SPENDER, amount, expiry)
    })
    .assert_success();
    env.advance_block();
}
fn height(env: &TestEnvironmentHandle) -> u32 {
    let r =
        env.transact(|tx| tx.call(&TOKEN).run(sdk::block::get_block_height));
    r.assert_success();
    u32::try_from(*r.assert_return_as::<BlockHeight>()).unwrap()
}
fn assert_state(
    env: &TestEnvironmentHandle,
    alice: i128,
    bob: i128,
    allowance: i128,
    supply: i128,
) {
    call(env, &OTHER, || {
        assert_eq!(token().balance(&ALICE), alice);
        assert_eq!(token().balance(&BOB), bob);
        assert_eq!(token().allowance(&ALICE, &SPENDER), allowance);
        assert_eq!(admin().total_supply(), supply);
    })
    .assert_success();
}
fn assert_error(r: TransactResult, expected: TokenError) {
    assert_eq!(r.assert_abort_as::<TokenError>(), expected);
    r.assert_no_events();
}
fn assert_amount_event<E: Event>(
    r: &TransactResult,
    addresses: &[ContractAddress],
    amount: i128,
) {
    r.assert_event_count(1);
    r.assert_raw_event(0, |event| {
        assert_eq!(event.topic(0), E::signature());
        for (i, address) in addresses.iter().enumerate() {
            assert_eq!(event.topic((i + 1) as u32), Some(&**address));
        }
        assert!(event.topic((addresses.len() + 1) as u32).is_none());
        assert_eq!(
            sdk::xdr::cxs::decode::<i128>(event.payload().unwrap()).unwrap(),
            amount
        );
    });
}

#[test]
fn deploy_and_initialize_atomically() {
    let env = TestEnvironment::no_cost();
    let (_, hash) = register(&env);
    let result = env.transact(|tx| {
        tx.call(&ADMIN).run(|| {
            let address = deploy_contract(&NONCE, &hash);
            let a = TokenAdminClient::new(&address);
            a.initialize(
                &MINTER,
                "Reference Token".try_into().unwrap(),
                "REF".try_into().unwrap(),
                7,
                open_policy(),
            );
            assert_eq!(a.admin(), ADMIN);
            assert_eq!(a.minter(), MINTER);
            address
        })
    });
    result.assert_success();
    let address: ContractAddress = result.assert_return_as();
    env.advance_block();
    call(&env, &OTHER, || {
        let t = TokenClient::new(&address);
        assert_eq!(t.name().as_str(), "Reference Token");
        assert_eq!(t.symbol().as_str(), "REF");
        assert_eq!(t.decimals(), 7);
        assert_eq!(t.balance(&ALICE), 0);
        assert_eq!(t.allowance(&ALICE, &SPENDER), 0);
        assert_eq!(TokenAdminClient::new(&address).total_supply(), 0);
    })
    .assert_success();
    assert_error(
        call(&env, &OTHER, || {
            TokenAdminClient::new(&address).initialize(
                &OTHER,
                "x".try_into().unwrap(),
                "x".try_into().unwrap(),
                0,
                open_policy(),
            );
        }),
        TokenError::AlreadyInitialized,
    );
}

#[test]
fn stellar_asset_metadata_round_trips_at_the_symbol_boundary() {
    // Valid SAC names are code + ':' + a 56-character issuer address.
    for symbol in ["ABCDEFGHIJK", "ABCDEFGHIJKL"] {
        let env = TestEnvironment::no_cost();
        let (contract, _) = register(&env);
        env.setup_create_deployment(&TOKEN, &contract);
        let name = format!("{symbol}:{}", "G".repeat(56));
        call(&env, &ADMIN, || {
            admin().initialize(
                &MINTER,
                name.as_str().try_into().unwrap(),
                symbol.try_into().unwrap(),
                7,
                open_policy(),
            );
        })
        .assert_success();
        env.advance_block();
        call(&env, &OTHER, || {
            assert_eq!(token().name().as_str(), name);
            assert_eq!(token().symbol().as_str(), symbol);
            assert_eq!(token().decimals(), 7);
        })
        .assert_success();
    }
    assert!(Symbol::try_from("ABCDEFGHIJKLM").is_err());
}

#[test]
fn initialization_is_required_and_invalid_policy_rolls_back() {
    let env = TestEnvironment::no_cost();
    let (contract, _) = register(&env);
    env.setup_create_deployment(&TOKEN, &contract);
    assert_error(
        call(&env, &ALICE, || {
            token().balance(&ALICE);
        }),
        TokenError::NotInitialized,
    );
    assert_error(
        call(&env, &ALICE, || token().approve(&ALICE, &SPENDER, 1, 100)),
        TokenError::NotInitialized,
    );
    assert_error(
        call(&env, &ADMIN, || {
            admin().initialize(
                &MINTER,
                "x".try_into().unwrap(),
                "x".try_into().unwrap(),
                0,
                Policy {
                    auth_required: false,
                    auth_revocable: false,
                    clawback_enabled: true,
                },
            );
        }),
        TokenError::InvalidPolicy,
    );
    call(&env, &ADMIN, init).assert_success();
    env.advance_block();
    assert_state(&env, 0, 0, 0, 0);
}

#[test]
fn absolute_approval_multiple_spends_and_revocation() {
    let env = setup();
    mint(&env, &ALICE, 200);
    let grant = call(&env, &ALICE, || {
        token().approve(&ALICE, &SPENDER, 100, 1000)
    });
    grant.assert_success();
    grant.assert_event_count(1);
    grant.assert_raw_event(0, |e| {
        assert_eq!(e.topic(0), Some(ApproveEvent::SIGNATURE));
        assert_eq!(e.topic(1), Some(&*ALICE));
        assert_eq!(e.topic(2), Some(&*SPENDER));
        assert_eq!(
            sdk::xdr::cxs::decode::<(i128, u32)>(e.payload().unwrap())
                .unwrap(),
            (100, 1000)
        );
    });
    env.advance_block();
    for amount in [30, 20] {
        let r = call(&env, &SPENDER, || {
            token().transfer_from(&SPENDER, &ALICE, &BOB, amount)
        });
        r.assert_success();
        assert_amount_event::<TransferEvent>(&r, &[ALICE, BOB], amount);
        env.advance_block();
    }
    assert_state(&env, 150, 50, 50, 200);
    approve(&env, 80, 2000); // Replace 50, do not add 80.
    assert_state(&env, 150, 50, 80, 200);
    approve(&env, 0, 0); // Past expiry is allowed for revocation.
    assert_state(&env, 150, 50, 0, 200);
    assert_error(
        call(&env, &SPENDER, || {
            token().transfer_from(&SPENDER, &ALICE, &BOB, 1)
        }),
        TokenError::InsufficientAllowance,
    );
}

#[test]
fn allowance_expiry_is_inclusive_and_spending_does_not_extend_it() {
    let env = setup();
    mint(&env, &ALICE, 100);
    let expiry = height(&env) + 1;
    approve(&env, 40, expiry);
    assert_eq!(height(&env), expiry);
    call(&env, &SPENDER, || {
        token().transfer_from(&SPENDER, &ALICE, &BOB, 10)
    })
    .assert_success();
    env.advance_block();
    assert_state(&env, 90, 10, 0, 100);
    assert_error(
        call(&env, &SPENDER, || token().burn_from(&SPENDER, &ALICE, 1)),
        TokenError::InsufficientAllowance,
    );
    assert_error(
        call(&env, &ALICE, || {
            token().approve(&ALICE, &SPENDER, 1, expiry)
        }),
        TokenError::InvalidExpiry,
    );
    approve(&env, 5, u32::MAX);
    assert_state(&env, 90, 10, 5, 100);
}

#[test]
fn expiry_at_current_height_allows_calls_in_the_same_transaction() {
    let env = setup();
    mint(&env, &ALICE, 100);
    call(&env, &ALICE, || {
        let now = u32::try_from(*sdk::block::get_block_height()).unwrap();
        token().approve(&ALICE, &ALICE, 20, now);
        token().transfer_from(&ALICE, &ALICE, &BOB, 10);
        token().burn_from(&ALICE, &ALICE, 10);
        assert_eq!(token().allowance(&ALICE, &ALICE), 0);
    })
    .assert_success();
    env.advance_block();
    assert_state(&env, 80, 10, 0, 90);
}

#[test]
fn callers_cannot_name_another_owner_or_spender() {
    let env = setup();
    mint(&env, &ALICE, 100);
    approve(&env, 50, 1000);
    for r in [
        call(&env, &OTHER, || {
            token().approve(&ALICE, &SPENDER, 100, 1000)
        }),
        call(&env, &OTHER, || token().transfer(&ALICE, &BOB, 10)),
        call(&env, &OTHER, || {
            token().transfer_from(&SPENDER, &ALICE, &BOB, 10)
        }),
        call(&env, &OTHER, || token().burn(&ALICE, 10)),
        call(&env, &OTHER, || token().burn_from(&SPENDER, &ALICE, 10)),
    ] {
        assert_error(r, TokenError::Unauthorized);
    }
    env.advance_block();
    assert_state(&env, 100, 0, 50, 100);
}

#[test]
fn negative_amounts_fail_in_every_amount_operation() {
    let env = setup();
    mint(&env, &ALICE, 100);
    approve(&env, 50, 1000);
    for r in [
        call(&env, &ALICE, || token().approve(&ALICE, &SPENDER, -1, 1000)),
        call(&env, &ALICE, || token().transfer(&ALICE, &BOB, i128::MIN)),
        call(&env, &SPENDER, || {
            token().transfer_from(&SPENDER, &ALICE, &BOB, -1)
        }),
        call(&env, &ALICE, || token().burn(&ALICE, -1)),
        call(&env, &SPENDER, || token().burn_from(&SPENDER, &ALICE, -1)),
        call(&env, &MINTER, || admin().mint(&ALICE, -1)),
        call(&env, &ADMIN, || admin().clawback(&ALICE, -1)),
    ] {
        assert_error(r, TokenError::NegativeAmount);
    }
    env.advance_block();
    assert_state(&env, 100, 0, 50, 100);
}

#[test]
fn allowance_does_not_reserve_balance_and_failed_spends_roll_back() {
    let env = setup();
    mint(&env, &ALICE, 10);
    approve(&env, 100, 1000);
    for r in [
        call(&env, &SPENDER, || {
            token().transfer_from(&SPENDER, &ALICE, &BOB, 11)
        }),
        call(&env, &SPENDER, || token().burn_from(&SPENDER, &ALICE, 11)),
    ] {
        assert_error(r, TokenError::InsufficientBalance);
    }
    env.advance_block();
    assert_state(&env, 10, 0, 100, 10);
    // Catching a callee exception must also roll back its allowance debit.
    call(&env, &SPENDER, || {
        assert_eq!(
            token()
                .try_transfer_from(&SPENDER, &ALICE, &BOB, 11)
                .unwrap_err()
                .try_as(),
            Some(TokenError::InsufficientBalance)
        );
        token().transfer_from(&SPENDER, &ALICE, &BOB, 10);
    })
    .assert_success();
    env.advance_block();
    assert_state(&env, 0, 10, 90, 10);
}

#[test]
fn holder_burn_delegated_burn_and_clawback_have_distinct_authority_and_events()
{
    let env = setup();
    mint(&env, &ALICE, 100);
    approve(&env, 50, 1000);
    let r = call(&env, &ALICE, || token().burn(&ALICE, 10));
    r.assert_success();
    assert_amount_event::<BurnEvent>(&r, &[ALICE], 10);
    env.advance_block();
    let r = call(&env, &SPENDER, || token().burn_from(&SPENDER, &ALICE, 20));
    r.assert_success();
    assert_amount_event::<BurnEvent>(&r, &[ALICE], 20);
    env.advance_block();
    let r = call(&env, &ADMIN, || admin().clawback(&ALICE, 30));
    r.assert_success();
    assert_amount_event::<ClawbackEvent>(&r, &[ALICE], 30);
    env.advance_block();
    assert_state(&env, 40, 0, 30, 40);
    assert_error(
        call(&env, &ADMIN, || token().burn(&ALICE, 1)),
        TokenError::Unauthorized,
    );
    assert_error(
        call(&env, &ALICE, || admin().clawback(&ALICE, 1)),
        TokenError::Unauthorized,
    );
}

#[test]
fn freeze_blocks_all_holder_movements_but_allows_approval_and_clawback() {
    let env = setup();
    mint(&env, &ALICE, 100);
    approve(&env, 50, 1000);
    let r = call(&env, &ADMIN, || admin().set_authorized(&ALICE, false));
    r.assert_success();
    r.assert_event_count(1);
    r.assert_raw_event(0, |e| {
        assert_eq!(e.topic(0), Some(SetAuthorizedEvent::SIGNATURE));
        assert_eq!(e.topic(1), Some(&*ALICE));
        assert!(!sdk::xdr::cxs::decode::<bool>(e.payload().unwrap()).unwrap());
    });
    env.advance_block();
    for r in [
        call(&env, &ALICE, || token().transfer(&ALICE, &BOB, 1)),
        call(&env, &BOB, || token().transfer(&BOB, &ALICE, 0)),
        call(&env, &SPENDER, || {
            token().transfer_from(&SPENDER, &ALICE, &BOB, 1)
        }),
        call(&env, &ALICE, || token().burn(&ALICE, 1)),
        call(&env, &SPENDER, || token().burn_from(&SPENDER, &ALICE, 1)),
        call(&env, &MINTER, || admin().mint(&ALICE, 1)),
    ] {
        assert_error(r, TokenError::HolderNotAuthorized);
    }
    approve(&env, 60, 1000);
    approve(&env, 0, 0);
    call(&env, &ADMIN, || admin().clawback(&ALICE, 10)).assert_success();
    env.advance_block();
    assert_state(&env, 90, 0, 0, 90);
    call(&env, &ADMIN, || admin().set_authorized(&ALICE, true))
        .assert_success();
    env.advance_block();
    call(&env, &ALICE, || token().transfer(&ALICE, &BOB, 10)).assert_success();
    env.advance_block();
    assert_state(&env, 80, 10, 0, 90);
}

#[test]
fn required_authorization_defaults_closed_and_checks_destination() {
    let env = setup_with(Policy {
        auth_required: true,
        auth_revocable: true,
        clawback_enabled: true,
    });
    call(&env, &OTHER, || assert!(!admin().authorized(&ALICE)))
        .assert_success();
    assert_error(
        call(&env, &MINTER, || admin().mint(&ALICE, 10)),
        TokenError::HolderNotAuthorized,
    );
    call(&env, &ADMIN, || admin().set_authorized(&ALICE, true))
        .assert_success();
    env.advance_block();
    mint(&env, &ALICE, 10);
    approve(&env, 10, 1000);
    assert_error(
        call(&env, &SPENDER, || {
            token().transfer_from(&SPENDER, &ALICE, &BOB, 5)
        }),
        TokenError::HolderNotAuthorized,
    );
    env.advance_block();
    assert_state(&env, 10, 0, 10, 10);
    call(&env, &ADMIN, || admin().set_authorized(&BOB, true)).assert_success();
    env.advance_block();
    call(&env, &SPENDER, || {
        token().transfer_from(&SPENDER, &ALICE, &BOB, 5)
    })
    .assert_success();
    env.advance_block();
    assert_state(&env, 5, 5, 5, 10);
}

#[test]
fn policy_disables_revocation_and_clawback() {
    let env = setup_with(Policy {
        auth_required: false,
        auth_revocable: false,
        clawback_enabled: false,
    });
    mint(&env, &ALICE, 10);
    assert_error(
        call(&env, &ADMIN, || admin().set_authorized(&ALICE, false)),
        TokenError::OperationDisabled,
    );
    assert_error(
        call(&env, &ADMIN, || admin().clawback(&ALICE, 1)),
        TokenError::OperationDisabled,
    );
    env.advance_block();
    assert_state(&env, 10, 0, 0, 10);
}

#[test]
fn minter_and_admin_authority_are_separate_and_rotatable() {
    let env = setup();
    assert_error(
        call(&env, &ADMIN, || admin().mint(&ALICE, 1)),
        TokenError::Unauthorized,
    );
    for r in [
        call(&env, &MINTER, || admin().set_authorized(&ALICE, false)),
        call(&env, &MINTER, || admin().clawback(&ALICE, 0)),
        call(&env, &MINTER, || admin().set_admin(&MINTER)),
        call(&env, &MINTER, || admin().set_minter(&OTHER)),
    ] {
        assert_error(r, TokenError::Unauthorized);
    }
    let r = call(&env, &ADMIN, || {
        admin().set_minter(&OTHER);
        admin().set_admin(&BOB);
    });
    r.assert_success();
    r.assert_event_count(2);
    r.assert_raw_event(0, |e| {
        assert_eq!(e.topic(0), Some(SetMinterEvent::SIGNATURE));
        assert_eq!(e.topic(1), Some(&*MINTER));
        assert_eq!(
            sdk::xdr::cxs::decode::<ContractAddress>(e.payload().unwrap())
                .unwrap(),
            OTHER
        );
    });
    r.assert_raw_event(1, |e| {
        assert_eq!(e.topic(0), Some(SetAdminEvent::SIGNATURE));
        assert_eq!(e.topic(1), Some(&*ADMIN));
        assert_eq!(
            sdk::xdr::cxs::decode::<ContractAddress>(e.payload().unwrap())
                .unwrap(),
            BOB
        );
    });
    env.advance_block();
    assert_error(
        call(&env, &MINTER, || admin().mint(&ALICE, 1)),
        TokenError::Unauthorized,
    );
    assert_error(
        call(&env, &ADMIN, || admin().set_minter(&MINTER)),
        TokenError::Unauthorized,
    );
    let r = call(&env, &OTHER, || admin().mint(&ALICE, 5));
    r.assert_success();
    assert_amount_event::<MintEvent>(&r, &[ALICE], 5);
    env.advance_block();
    assert_state(&env, 5, 0, 0, 5);
}

#[test]
fn maximum_supply_and_overflow_roll_back() {
    let env = setup();
    mint(&env, &ALICE, i128::MAX);
    assert_error(
        call(&env, &MINTER, || admin().mint(&BOB, 1)),
        TokenError::Overflow,
    );
    env.advance_block();
    assert_state(&env, i128::MAX, 0, 0, i128::MAX);
    approve(&env, i128::MAX, u32::MAX);
    call(&env, &SPENDER, || {
        token().transfer_from(&SPENDER, &ALICE, &BOB, i128::MAX)
    })
    .assert_success();
    env.advance_block();
    assert_state(&env, 0, i128::MAX, 0, i128::MAX);
    call(&env, &BOB, || token().burn(&BOB, i128::MAX)).assert_success();
    env.advance_block();
    assert_state(&env, 0, 0, 0, 0);
}

#[test]
fn zero_operations_emit_events_and_self_transfer_preserves_balance() {
    let env = setup();
    mint(&env, &ALICE, 10);
    let r = call(&env, &ALICE, || token().transfer(&ALICE, &ALICE, 10));
    r.assert_success();
    assert_amount_event::<TransferEvent>(&r, &[ALICE, ALICE], 10);
    env.advance_block();
    let r = call(&env, &ALICE, || token().transfer(&ALICE, &BOB, 0));
    r.assert_success();
    assert_amount_event::<TransferEvent>(&r, &[ALICE, BOB], 0);
    let r = call(&env, &SPENDER, || token().burn_from(&SPENDER, &ALICE, 0));
    r.assert_success();
    assert_amount_event::<BurnEvent>(&r, &[ALICE], 0);
    env.advance_block();
    assert_state(&env, 10, 0, 0, 10);
}

#[test]
fn equal_same_block_spends_cannot_merge_allowance_writes() {
    let env = setup();
    mint(&env, &ALICE, 100);
    approve(&env, 50, 1000);
    call(&env, &SPENDER, || {
        token().transfer_from(&SPENDER, &ALICE, &BOB, 10)
    })
    .assert_success();
    let duplicate = call(&env, &SPENDER, || {
        token().transfer_from(&SPENDER, &ALICE, &BOB, 10)
    });
    duplicate.assert_commit_failed_nondeterministically();
    env.advance_block();
    assert_state(&env, 90, 10, 40, 100);
    call(&env, &SPENDER, || {
        token().transfer_from(&SPENDER, &ALICE, &BOB, 10)
    })
    .assert_success();
    env.advance_block();
    assert_state(&env, 80, 20, 30, 100);
}

#[test]
fn replacing_or_revoking_an_allowance_conflicts_with_same_block_spending() {
    for (replacement, approval_first) in
        [(40, true), (40, false), (0, true), (0, false)]
    {
        let env = setup();
        mint(&env, &ALICE, 100);
        approve(&env, 50, 1000);
        let grant = || {
            call(&env, &ALICE, || {
                token().approve(&ALICE, &SPENDER, replacement, 1000)
            })
        };
        let spend = || {
            call(&env, &SPENDER, || {
                token().transfer_from(&SPENDER, &ALICE, &BOB, 10)
            })
        };
        if approval_first {
            grant().assert_success();
            spend().assert_commit_failed_nondeterministically();
            env.advance_block();
            assert_state(&env, 100, 0, replacement, 100);
        } else {
            spend().assert_success();
            grant().assert_commit_failed_nondeterministically();
            env.advance_block();
            assert_state(&env, 90, 10, 40, 100);
        }
    }
}

#[test]
fn separate_allowance_pairs_can_spend_in_one_block() {
    let env = setup();
    mint(&env, &ALICE, 100);
    call(&env, &MINTER, || admin().mint(&OTHER, 100)).assert_success();
    env.advance_block();
    call(&env, &ALICE, || token().approve(&ALICE, &SPENDER, 50, 1000))
        .assert_success();
    call(&env, &OTHER, || token().approve(&OTHER, &SPENDER, 50, 1000))
        .assert_success();
    env.advance_block();
    call(&env, &SPENDER, || {
        token().transfer_from(&SPENDER, &ALICE, &BOB, 10)
    })
    .assert_success();
    call(&env, &SPENDER, || {
        token().transfer_from(&SPENDER, &OTHER, &BOB, 10)
    })
    .assert_success();
    env.advance_block();
    assert_state(&env, 90, 20, 40, 200);
    call(&env, &OTHER, || {
        assert_eq!(token().balance(&OTHER), 90);
        assert_eq!(token().allowance(&OTHER, &SPENDER), 40);
    })
    .assert_success();
}

#[test]
fn freeze_takes_effect_next_block_not_retroactively() {
    let env = setup();
    mint(&env, &ALICE, 100);
    approve(&env, 50, 1000);
    call(&env, &ADMIN, || admin().set_authorized(&ALICE, false))
        .assert_success();
    // The spender still sees the starting snapshot in this block.
    call(&env, &SPENDER, || {
        token().transfer_from(&SPENDER, &ALICE, &BOB, 10)
    })
    .assert_success();
    env.advance_block();
    assert_state(&env, 90, 10, 40, 100);
    assert_error(
        call(&env, &SPENDER, || {
            token().transfer_from(&SPENDER, &ALICE, &BOB, 1)
        }),
        TokenError::HolderNotAuthorized,
    );
}

#[test]
fn separate_allowances_cannot_overspend_one_owners_balance() {
    let env = setup();
    mint(&env, &ALICE, 100);
    call(&env, &ALICE, || {
        token().approve(&ALICE, &SPENDER, 80, 1000);
        token().approve(&ALICE, &OTHER, 80, 1000);
    })
    .assert_success();
    env.advance_block();
    call(&env, &SPENDER, || {
        token().transfer_from(&SPENDER, &ALICE, &BOB, 60)
    })
    .assert_success();
    let losing = call(&env, &OTHER, || {
        token().transfer_from(&OTHER, &ALICE, &BOB, 60)
    });
    losing.assert_commit_failed_nondeterministically();
    env.advance_block();
    assert_state(&env, 40, 60, 20, 100);
    call(&env, &OTHER, || {
        assert_eq!(token().allowance(&ALICE, &OTHER), 80)
    })
    .assert_success();
}

#[test]
fn movements_decode_with_existing_groundhog_token_events() {
    // Use the consumer's crate directly: copying fields into new SEP-41 event
    // definitions must not silently change the signatures used by token history.
    use groundhog_erc20_if as existing;

    let env = setup();
    let r = call(&env, &MINTER, || admin().mint(&ALICE, 100));
    r.assert_success();
    assert_amount_event::<existing::MintEvent>(&r, &[ALICE], 100);
    r.assert_raw_event(0, |e| {
        let decoded =
            existing::MintEvent::try_from(EmittedEvent::from(e.raw()))
                .unwrap();
        assert_eq!((*decoded.to, decoded.amount), (ALICE, 100));
    });
    env.advance_block();

    let r = call(&env, &ALICE, || token().approve(&ALICE, &SPENDER, 50, 1000));
    r.assert_success();
    r.assert_event_count(1);
    r.assert_raw_event(0, |e| {
        // ERC20 approvals are deltas; SEP-41 grants are absolute and expire.
        assert!(
            existing::ApprovalEvent::try_from(EmittedEvent::from(e.raw()))
                .is_err()
        );
    });
    env.advance_block();

    for delegated in [false, true] {
        let caller = if delegated { &SPENDER } else { &ALICE };
        let r = call(&env, caller, || {
            if delegated {
                token().transfer_from(&SPENDER, &ALICE, &BOB, 10);
            } else {
                token().transfer(&ALICE, &BOB, 10);
            }
        });
        r.assert_success();
        assert_amount_event::<existing::TransferEvent>(&r, &[ALICE, BOB], 10);
        r.assert_raw_event(0, |e| {
            let decoded =
                existing::TransferEvent::try_from(EmittedEvent::from(e.raw()))
                    .unwrap();
            assert_eq!(
                (*decoded.from, *decoded.to, decoded.amount),
                (ALICE, BOB, 10)
            );
        });
        env.advance_block();

        let r = call(&env, caller, || {
            if delegated {
                token().burn_from(&SPENDER, &ALICE, 5);
            } else {
                token().burn(&ALICE, 5);
            }
        });
        r.assert_success();
        assert_amount_event::<existing::BurnEvent>(&r, &[ALICE], 5);
        r.assert_raw_event(0, |e| {
            let decoded =
                existing::BurnEvent::try_from(EmittedEvent::from(e.raw()))
                    .unwrap();
            assert_eq!((*decoded.from, decoded.amount), (ALICE, 5));
        });
        env.advance_block();
    }

    let r = call(&env, &ADMIN, || admin().clawback(&ALICE, 1));
    r.assert_success();
    assert_amount_event::<ClawbackEvent>(&r, &[ALICE], 1);
    r.assert_raw_event(0, |e| {
        assert!(
            existing::BurnEvent::try_from(EmittedEvent::from(e.raw()))
                .is_err()
        );
    });
    env.advance_block();
    assert_state(&env, 69, 20, 35, 89);
}

/// Export actual runtime events for the CLI's HTTP consumer tests.
/// Only contract event bytes are exported; the consumer supplies test ledger IDs.
#[test]
#[ignore = "requires SEP41_CLI_EVENTS_OUT; optionally SEP41_RV32_ARTIFACT"]
fn export_cli_event_fixture() {
    let output = std::env::var_os("SEP41_CLI_EVENTS_OUT")
        .expect("set SEP41_CLI_EVENTS_OUT to the destination JSON path");
    let env = setup();
    type Operation = (&'static str, &'static ContractAddress, fn());
    let operations: [Operation; 8] = [
        ("mint", &MINTER, || admin().mint(&ALICE, 100)),
        ("transfer", &ALICE, || token().transfer(&ALICE, &BOB, 25)),
        ("approve", &ALICE, || {
            token().approve(&ALICE, &SPENDER, 30, 1000)
        }),
        ("burn", &ALICE, || token().burn(&ALICE, 5)),
        ("clawback", &ADMIN, || admin().clawback(&ALICE, 7)),
        ("set_authorized", &ADMIN, || {
            admin().set_authorized(&ALICE, false)
        }),
        ("set_minter", &ADMIN, || admin().set_minter(&OTHER)),
        ("set_admin", &ADMIN, || admin().set_admin(&OTHER)),
    ];
    let mut records = Vec::new();
    for (operation, caller, invoke) in operations {
        let result = call(&env, caller, invoke);
        result.assert_success();
        result.assert_event_count(1);
        result.assert_raw_event(0, |event| {
            let topics: Vec<_> = (0..event.num_topics())
                .map(|i| format!("\"0x{}\"", sdk::hex_encode(event.topic(i).unwrap())))
                .collect();
            records.push(format!(
                "  {{\"operation\":\"{operation}\",\"event\":{{\"contractAddress\":\"0x{}\",\"topics\":[{}],\"data\":\"0x{}\"}}}}",
                sdk::hex_encode(&event.raw().contract_address),
                topics.join(","),
                sdk::hex_encode(&event.payload().unwrap()),
            ));
        });
        env.advance_block();
    }
    std::fs::write(output, format!("[\n{}\n]\n", records.join(",\n")))
        .unwrap();
}
