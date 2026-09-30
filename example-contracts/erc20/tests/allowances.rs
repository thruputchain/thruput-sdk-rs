use erc20::*;
use sdk::*;

const TOKEN: ContractAddress = ContractAddress::new([0x31; 32]);
const OWNER: ContractAddress = ContractAddress::new([0x32; 32]);
const SPENDER: ContractAddress = ContractAddress::new([0x33; 32]);
const OTHER: ContractAddress = ContractAddress::new([0x34; 32]);
fn client() -> ERC20Client {
    ERC20Client::new(&TOKEN)
}
fn expiry() -> groundhog_erc20_if::ERC20AllowanceExpiryClient {
    groundhog_erc20_if::ERC20AllowanceExpiryClient::new(&TOKEN)
}
fn call(
    env: &TestEnvironmentHandle,
    who: &ContractAddress,
    f: impl FnOnce() + std::panic::UnwindSafe,
) -> TransactResult {
    env.transact(|tx| tx.call(who).run(f))
}
fn setup() -> TestEnvironmentHandle {
    let env = TestEnvironment::no_cost();
    let contract = if let Some(path) = std::env::var_os("ERC20_RV32_ARTIFACT")
    {
        sdk::types::Contract::kRv32 {
            rv32_contract: sdk::types::contract::ContractBlob {
                data: sdk::xdr::BytesM(
                    std::fs::read(path).expect("read ERC20 RV32 artifact"),
                ),
            },
        }
    } else {
        erc20::register_mock_contract(&env).as_contract()
    };
    env.setup_create_contract(&contract);
    env.setup_create_deployment(&TOKEN, &contract);
    call(&env, &OWNER, || {
        client().initialize(&OWNER);
        client().mint(&OWNER, 100);
    })
    .assert_success();
    env.advance_block();
    call(&env, &OWNER, || {
        client().adjust_allowance(&SPENDER, 50);
    })
    .assert_success();
    env.advance_block();
    env
}

#[test]
fn absolute_sets_conflict_with_spending_in_both_orders() {
    for adjustment_first in [false, true] {
        for target in [0, 40, 50, 60] {
            let env = setup();
            let adjust = || {
                call(&env, &OWNER, || {
                    client().delete_allowance(&SPENDER);
                    client().adjust_allowance(&SPENDER, target);
                })
            };
            let spend = || {
                call(&env, &SPENDER, || {
                    client().transfer_from(&OWNER, &OTHER, 10)
                })
            };
            let (first, second) = if adjustment_first {
                (adjust(), spend())
            } else {
                (spend(), adjust())
            };
            first.assert_success();
            second.assert_commit_failed_nondeterministically();
            env.advance_block();
            call(&env, &OWNER, || {
                assert_eq!(
                    client().allowance(&OWNER, &SPENDER),
                    if adjustment_first { target as u128 } else { 40 }
                );
                assert_eq!(
                    client().get_balance_of(&OTHER),
                    if adjustment_first { 0 } else { 10 }
                );
            })
            .assert_success();
        }
    }
}

#[test]
fn absolute_sets_conflict_but_independent_spenders_can_commit() {
    let env = setup();
    let set = |spender: ContractAddress, amount| {
        call(&env, &OWNER, move || {
            client().delete_allowance(&spender);
            client().adjust_allowance(&spender, amount);
        })
    };
    set(SPENDER, 20).assert_success();
    set(SPENDER, 20).assert_commit_failed_nondeterministically();
    set(OTHER, 30).assert_success();
    env.advance_block();
    call(&env, &OWNER, || {
        assert_eq!(client().allowance(&OWNER, &SPENDER), 20);
        assert_eq!(client().allowance(&OWNER, &OTHER), 30);
    })
    .assert_success();
}

#[test]
fn partial_spends_merge_and_failed_spend_leaves_allowance_intact() {
    let env = setup();
    for _ in 0..2 {
        call(&env, &SPENDER, || {
            client().transfer_from(&OWNER, &OTHER, 10)
        })
        .assert_success();
    }
    env.advance_block();
    call(&env, &OWNER, || client().adjust_allowance(&SPENDER, 970))
        .assert_success();
    env.advance_block();
    call(&env, &SPENDER, || {
        assert_eq!(
            client()
                .try_transfer_from(&OWNER, &OTHER, 100)
                .unwrap_err()
                .try_as(),
            Some(groundhog_erc20_if::ERC20Error::TooLarge)
        );
        assert_eq!(client().allowance(&OWNER, &SPENDER), 1000);
    })
    .assert_success();
    call(&env, &OWNER, || client().delete_allowance(&SPENDER))
        .assert_success();
    env.advance_block();
    call(&env, &SPENDER, || {
        assert_eq!(client().allowance(&OWNER, &SPENDER), 0);
        assert_eq!(client().get_balance_of(&OTHER), 20);
    })
    .assert_success();
}

#[test]
fn adjustment_emits_delta_at_the_amount_bound() {
    use groundhog_erc20_if::AllowanceChangeEvent;
    let env = setup();
    let result = call(&env, &OWNER, || {
        client().adjust_allowance(&SPENDER, i128::MAX - 50);
        assert_eq!(client().allowance(&OWNER, &SPENDER), i128::MAX as u128);
    });
    result.assert_success();
    result.assert_event_count(1);
    result.assert_raw_event(0, |event| {
        let event =
            AllowanceChangeEvent::try_from(EmittedEvent::from(event.raw()))
                .unwrap();
        assert_eq!(event.owner, &OWNER);
        assert_eq!(event.spender, &SPENDER);
        assert_eq!(event.difference, i128::MAX - 50);
    });
}

#[test]
fn adjustment_rejects_overflow_without_writing() {
    let env = setup();
    let result = call(&env, &OWNER, || {
        client().adjust_allowance(&SPENDER, i128::MAX)
    });
    assert_eq!(
        result.assert_abort_as::<groundhog_erc20_if::ERC20Error>(),
        groundhog_erc20_if::ERC20Error::TooLarge
    );
    result.assert_no_events();
    env.advance_block();
    call(&env, &OWNER, || {
        assert_eq!(client().allowance(&OWNER, &SPENDER), 50)
    })
    .assert_success();
}

#[test]
fn signed_deltas_merge_and_spending_emits_only_a_transfer() {
    let env = setup();
    for difference in [20, -10, 0, -5] {
        let result = call(&env, &OWNER, || {
            client().adjust_allowance(&SPENDER, difference)
        });
        result.assert_success();
        result.assert_event_count(1);
        result.assert_raw_event(0, |raw| {
            let event = groundhog_erc20_if::AllowanceChangeEvent::try_from(
                EmittedEvent::from(raw.raw()),
            )
            .unwrap();
            assert_eq!(event.owner, &OWNER);
            assert_eq!(event.spender, &SPENDER);
            assert_eq!(event.difference, difference);
        });
    }
    let result = call(&env, &SPENDER, || {
        client().transfer_from(&OWNER, &OTHER, 10)
    });
    result.assert_success();
    result.assert_event_count(1);
    result.assert_raw_event(0, |raw| {
        assert_eq!(
            raw.topic(0),
            Some(groundhog_erc20_if::TransferEvent::SIGNATURE)
        );
    });
    env.advance_block();
    call(&env, &OWNER, || {
        assert_eq!(client().allowance(&OWNER, &SPENDER), 45)
    })
    .assert_success();
}

#[test]
fn clearing_conflicts_with_concurrent_grants_and_spends_in_both_orders() {
    for clear_first in [false, true] {
        for grant in [false, true] {
            let env = setup();
            let clear =
                || call(&env, &OWNER, || client().delete_allowance(&SPENDER));
            let change = || {
                if grant {
                    call(&env, &OWNER, || {
                        client().adjust_allowance(&SPENDER, 10)
                    })
                } else {
                    call(&env, &SPENDER, || {
                        client().transfer_from(&OWNER, &OTHER, 10)
                    })
                }
            };
            let (first, second) = if clear_first {
                (clear(), change())
            } else {
                (change(), clear())
            };
            first.assert_success();
            second.assert_commit_failed_nondeterministically();
            env.advance_block();
            call(&env, &OWNER, || {
                assert_eq!(
                    client().allowance(&OWNER, &SPENDER),
                    if clear_first {
                        0
                    } else if grant {
                        60
                    } else {
                        40
                    }
                )
            })
            .assert_success();
        }
    }
}

#[test]
fn temporary_grant_nested_spend_and_clear_commit_atomically() {
    let env = setup();
    call(&env, &OWNER, || client().delete_allowance(&SPENDER))
        .assert_success();
    env.advance_block();
    let entrypoint = sdk::contract::ContractEntrypointID::from_u64(0x1234);
    let mock = env.register_mock(|contract| {
        contract.register_method(entrypoint, || {
            client().transfer_from(&OWNER, &OTHER, 7);
            sdk::internal::contract_return_encoded(&());
        });
    });
    mock.deploy_contract_to(&SPENDER);
    let result = call(&env, &OWNER, || {
        client().adjust_allowance(&SPENDER, 10);
        sdk::call::try_call_raw_with::<(), (), _>(
            &SPENDER,
            entrypoint,
            u64::MAX,
            &(),
            0,
            |error| panic!("spender failed: {error:?}"),
        )
        .unwrap();
        client().delete_allowance(&SPENDER);
        assert_eq!(client().allowance(&OWNER, &SPENDER), 0);
    });
    result.assert_success();
    result.assert_event_count(3);
    result.assert_raw_event(2, |raw| {
        let event = groundhog_erc20_if::AllowanceDeleteEvent::try_from(
            EmittedEvent::from(raw.raw()),
        )
        .unwrap();
        assert_eq!(event.removed, 3);
    });
    env.advance_block();
    call(&env, &OWNER, || {
        assert_eq!(client().get_balance(), 93);
        assert_eq!(client().get_balance_of(&OTHER), 7);
        assert_eq!(client().allowance(&OWNER, &SPENDER), 0);
        client().delete_allowance(&SPENDER);
        client().delete_allowance(&SPENDER);
    })
    .assert_success();
}

// These keys deliberately follow the reference contract's storage slots so
// deletion is checked against committed storage, not just its zero-defaulting getter.
fn storage_key(
    env: &TestEnvironmentHandle,
    field: u32,
    account: &ContractAddress,
    spender: Option<(&ContractAddress, u32)>,
) -> StorageKey {
    let result = env.transact(|tx| {
        tx.call(&OTHER).run(|| {
            use sdk::storage::{Keypath, StorageDomain};
            let path = Keypath::root(TokenContract::DOMAIN)
                .child(&field)
                .child(account);
            // An allowance slot is a node: `amount` is field 0, `expiry` 1.
            match spender {
                Some((spender, field)) => {
                    path.child(spender).child(&field).into_key()
                }
                None => path.into_key(),
            }
        })
    });
    result.assert_success();
    result.assert_return_as()
}

#[test]
fn clearing_removes_allowance_and_zero_balance_storage_and_accepts_missing_cells()
 {
    let env = setup();
    let allowance_key = storage_key(&env, 2, &OWNER, Some((&SPENDER, 0)));
    let balance_key = storage_key(&env, 1, &OWNER, None);
    assert!(
        env.get_snapshot_storage_item(&TOKEN, &allowance_key)
            .is_some()
    );
    assert!(
        env.get_snapshot_storage_item(&TOKEN, &balance_key)
            .is_some()
    );
    let rejected = call(&env, &OWNER, || client().delete_balance());
    assert_eq!(
        rejected.assert_abort_as::<groundhog_erc20_if::ERC20Error>(),
        groundhog_erc20_if::ERC20Error::BalanceNotZero
    );
    rejected.assert_no_events();
    call(&env, &OWNER, || client().transfer(&OTHER, 100)).assert_success();
    env.advance_block();
    assert!(
        env.get_snapshot_storage_item(&TOKEN, &balance_key)
            .is_some()
    );
    call(&env, &OWNER, || {
        client().delete_allowance(&SPENDER);
        client().delete_balance();
    })
    .assert_success();
    env.advance_block();
    assert!(
        env.get_snapshot_storage_item(&TOKEN, &allowance_key)
            .is_none()
    );
    assert!(
        env.get_snapshot_storage_item(&TOKEN, &balance_key)
            .is_none()
    );
    call(&env, &OWNER, || {
        client().delete_allowance(&SPENDER);
        client().delete_balance();
        assert_eq!(client().get_balance(), 0);
        assert_eq!(client().allowance(&OWNER, &SPENDER), 0);
        assert_eq!(client().total_supply(), 100);
    })
    .assert_success();
}

#[test]
fn deleting_zero_balance_conflicts_with_concurrent_credit_in_both_orders() {
    for clear_first in [false, true] {
        for existing_zero in [false, true] {
            let env = setup();
            if existing_zero {
                call(&env, &OWNER, || client().transfer(&OTHER, 1))
                    .assert_success();
                env.advance_block();
                call(&env, &OTHER, || client().transfer(&OWNER, 1))
                    .assert_success();
                env.advance_block();
            }
            let clear = || call(&env, &OTHER, || client().delete_balance());
            let credit =
                || call(&env, &OWNER, || client().transfer(&OTHER, 10));
            let (first, second) = if clear_first {
                (clear(), credit())
            } else {
                (credit(), clear())
            };
            first.assert_success();
            second.assert_commit_failed_nondeterministically();
            env.advance_block();
            call(&env, &OWNER, || {
                assert_eq!(
                    client().get_balance_of(&OTHER),
                    if clear_first { 0 } else { 10 }
                );
                assert_eq!(
                    client().get_balance(),
                    if clear_first { 100 } else { 90 }
                );
            })
            .assert_success();
        }
    }
}

#[test]
fn clearing_a_missing_allowance_conflicts_with_a_new_grant_in_both_orders() {
    for clear_first in [false, true] {
        let env = setup();
        let clear =
            || call(&env, &OWNER, || client().delete_allowance(&OTHER));
        let grant =
            || call(&env, &OWNER, || client().adjust_allowance(&OTHER, 10));
        let (first, second) = if clear_first {
            (clear(), grant())
        } else {
            (grant(), clear())
        };
        first.assert_success();
        second.assert_commit_failed_nondeterministically();
        env.advance_block();
        call(&env, &OWNER, || {
            assert_eq!(
                client().allowance(&OWNER, &OTHER),
                if clear_first { 0 } else { 10 }
            )
        })
        .assert_success();
    }
}

#[test]
fn combined_adjustments_reject_overflow_and_overdraft() {
    for difference in [i128::MAX - 55, -30] {
        let env = setup();
        call(&env, &OWNER, || {
            client().adjust_allowance(&SPENDER, difference)
        })
        .assert_success();
        call(&env, &OWNER, || {
            client().adjust_allowance(&SPENDER, difference)
        })
        .assert_commit_failed_nondeterministically();
        env.advance_block();
        call(&env, &OWNER, || {
            assert_eq!(
                client().allowance(&OWNER, &SPENDER),
                (50 + difference) as u128
            );
        })
        .assert_success();
    }
}

#[test]
fn deleting_emits_the_removed_amount_and_allows_regrant_in_one_transaction() {
    let env = setup();
    let result = call(&env, &OWNER, || {
        client().delete_allowance(&SPENDER);
        client().delete_allowance(&SPENDER);
        client().adjust_allowance(&SPENDER, 12);
        assert_eq!(client().allowance(&OWNER, &SPENDER), 12);
    });
    result.assert_success();
    result.assert_event_count(3);
    for (index, removed) in [50, 0].into_iter().enumerate() {
        result.assert_raw_event(index, |raw| {
            let event = groundhog_erc20_if::AllowanceDeleteEvent::try_from(
                EmittedEvent::from(raw.raw()),
            )
            .unwrap();
            assert_eq!(event.owner, &OWNER);
            assert_eq!(event.spender, &SPENDER);
            assert_eq!(event.removed, removed);
        });
    }
    result.assert_raw_event(2, |raw| {
        let event = groundhog_erc20_if::AllowanceChangeEvent::try_from(
            EmittedEvent::from(raw.raw()),
        )
        .unwrap();
        assert_eq!(event.difference, 12);
    });
    env.advance_block();
    call(&env, &OWNER, || {
        assert_eq!(client().allowance(&OWNER, &SPENDER), 12);
    })
    .assert_success();
}

#[test]
fn clear_exclusivity_survives_regrant() {
    for clear_first in [true, false] {
        let env = setup();
        let clear = || {
            call(&env, &OWNER, || {
                client().delete_allowance(&OTHER);
                client().adjust_allowance(&OTHER, 50);
            })
        };
        let shared =
            || call(&env, &OWNER, || client().adjust_allowance(&OTHER, 10));
        let (first, second) = if clear_first {
            (clear(), shared())
        } else {
            (shared(), clear())
        };
        first.assert_success();
        second.assert_commit_failed_nondeterministically();
    }
}
#[test]
fn balance_delete_exclusivity_survives_later_credit() {
    for clear_first in [true, false] {
        let env = setup();
        call(&env, &OWNER, || client().adjust_allowance(&OTHER, 10))
            .assert_success();
        env.advance_block();
        let clear = || {
            call(&env, &OTHER, || {
                client().delete_balance();
                client().transfer_from(&OWNER, &OTHER, 1);
            })
        };
        let credit = || call(&env, &OWNER, || client().transfer(&OTHER, 10));
        let (first, second) = if clear_first {
            (clear(), credit())
        } else {
            (credit(), clear())
        };
        first.assert_success();
        second.assert_commit_failed_nondeterministically();
    }
}

/// The current height, read from inside a frame because that is the only
/// place the syscall answers.
fn current_height(env: &TestEnvironmentHandle) -> i64 {
    let result =
        env.transact(|tx| tx.call(&OWNER).run(sdk::block::get_block_height));
    result.assert_success();
    *result.assert_return_as::<BlockHeight>()
}

/// Replace the allowance with `amount`, spendable through `until`.
fn grant_until(env: &TestEnvironmentHandle, amount: i128, until: i64) {
    call(env, &OWNER, move || {
        expiry().reset_allowance(&SPENDER, until.into());
        client().adjust_allowance(&SPENDER, amount);
    })
    .assert_success();
    env.advance_block();
}

fn assert_spend_rejected(env: &TestEnvironmentHandle) {
    call(env, &SPENDER, || {
        assert_eq!(
            client()
                .try_transfer_from(&OWNER, &OTHER, 1)
                .unwrap_err()
                .try_as(),
            Some(groundhog_erc20_if::ERC20Error::TooLarge)
        );
    })
    .assert_success();
}

#[test]
fn expiry_is_inclusive_then_reads_zero_and_rejects_spending() {
    let env = setup();
    let height = current_height(&env);
    grant_until(&env, 50, height + 1);
    call(&env, &SPENDER, || {
        client().transfer_from(&OWNER, &OTHER, 10);
        assert_eq!(client().allowance(&OWNER, &SPENDER), 40);
    })
    .assert_success();
    env.advance_block();
    call(&env, &OWNER, || {
        assert_eq!(client().allowance(&OWNER, &SPENDER), 0)
    })
    .assert_success();
    assert_spend_rejected(&env);
}

#[test]
fn reset_zeroes_the_allowance_and_emits_the_removed_amount() {
    let env = setup();
    let height = current_height(&env);
    let result = call(&env, &OWNER, move || {
        expiry().reset_allowance(&SPENDER, (height + 10).into());
        assert_eq!(client().allowance(&OWNER, &SPENDER), 0);
    });
    result.assert_success();
    result.assert_event_count(1);
    result.assert_raw_event(0, |raw| {
        let event = groundhog_erc20_if::AllowanceResetEvent::try_from(
            EmittedEvent::from(raw.raw()),
        )
        .unwrap();
        assert_eq!(event.owner, &OWNER);
        assert_eq!(event.spender, &SPENDER);
        assert_eq!(event.removed, 50);
        assert_eq!(*event.until, height + 10);
    });
    env.advance_block();
    assert_spend_rejected(&env);
}

#[test]
fn reset_conflicts_with_spending_in_both_orders() {
    for reset_first in [false, true] {
        let env = setup();
        let height = current_height(&env);
        let reset = || {
            call(&env, &OWNER, move || {
                expiry().reset_allowance(&SPENDER, (height + 10).into());
                client().adjust_allowance(&SPENDER, 5);
            })
        };
        let spend = || {
            call(&env, &SPENDER, || {
                client().transfer_from(&OWNER, &OTHER, 10)
            })
        };
        let (first, second) = if reset_first {
            (reset(), spend())
        } else {
            (spend(), reset())
        };
        first.assert_success();
        second.assert_commit_failed_nondeterministically();
        env.advance_block();
        call(&env, &OWNER, || {
            assert_eq!(
                client().allowance(&OWNER, &SPENDER),
                if reset_first { 5 } else { 40 }
            )
        })
        .assert_success();
    }
}

#[test]
fn adjusting_an_expired_allowance_does_not_revive_it_but_deleting_does() {
    let env = setup();
    let expiry_key = storage_key(&env, 2, &OWNER, Some((&SPENDER, 1)));
    let height = current_height(&env);
    grant_until(&env, 50, height);
    assert!(env.get_snapshot_storage_item(&TOKEN, &expiry_key).is_some());
    call(&env, &OWNER, || {
        client().adjust_allowance(&SPENDER, 10);
        assert_eq!(client().allowance(&OWNER, &SPENDER), 0);
    })
    .assert_success();
    env.advance_block();
    assert_spend_rejected(&env);
    call(&env, &OWNER, || {
        client().delete_allowance(&SPENDER);
        client().adjust_allowance(&SPENDER, 10);
    })
    .assert_success();
    env.advance_block();
    assert!(env.get_snapshot_storage_item(&TOKEN, &expiry_key).is_none());
    call(&env, &SPENDER, || {
        assert_eq!(client().allowance(&OWNER, &SPENDER), 10);
        client().transfer_from(&OWNER, &OTHER, 10);
    })
    .assert_success();
}

#[test]
fn zero_amount_spends_succeed_after_expiry() {
    let env = setup();
    let height = current_height(&env);
    grant_until(&env, 50, height);
    call(&env, &SPENDER, || client().transfer_from(&OWNER, &OTHER, 0))
        .assert_success();
    assert_spend_rejected(&env);
}

#[test]
fn delete_reports_the_stored_amount_even_after_expiry() {
    let env = setup();
    let height = current_height(&env);
    grant_until(&env, 50, height);
    let result = call(&env, &OWNER, || {
        assert_eq!(client().allowance(&OWNER, &SPENDER), 0);
        client().delete_allowance(&SPENDER);
    });
    result.assert_success();
    result.assert_event_count(1);
    result.assert_raw_event(0, |raw| {
        let event = groundhog_erc20_if::AllowanceDeleteEvent::try_from(
            EmittedEvent::from(raw.raw()),
        )
        .unwrap();
        assert_eq!(event.removed, 50);
    });
}
