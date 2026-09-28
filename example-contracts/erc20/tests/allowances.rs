use erc20::*;
use sdk::*;

const TOKEN: ContractAddress = ContractAddress::new([0x31; 32]);
const OWNER: ContractAddress = ContractAddress::new([0x32; 32]);
const SPENDER: ContractAddress = ContractAddress::new([0x33; 32]);
const OTHER: ContractAddress = ContractAddress::new([0x34; 32]);
fn client() -> ERC20Client {
    ERC20Client::new(&TOKEN)
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
        client().adjust_allowance(&SPENDER, 50, false);
    })
    .assert_success();
    env.advance_block();
    env
}

#[test]
fn exclusive_adjustments_conflict_with_spending_in_both_orders() {
    for adjustment_first in [false, true] {
        for difference in [-50, -10, 0, 10] {
            let env = setup();
            let adjust = || {
                call(&env, &OWNER, || {
                    client().adjust_allowance(&SPENDER, difference, true)
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
                    if adjustment_first {
                        (50 + difference) as u128
                    } else {
                        40
                    }
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
fn exclusive_adjustments_conflict_but_independent_spenders_can_commit() {
    let env = setup();
    call(&env, &OWNER, || {
        client().adjust_allowance(&SPENDER, -30, true)
    })
    .assert_success();
    call(&env, &OWNER, || {
        client().adjust_allowance(&SPENDER, -30, true)
    })
    .assert_commit_failed_nondeterministically();
    call(&env, &OWNER, || client().adjust_allowance(&OTHER, 30, true))
        .assert_success();
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
    call(&env, &OWNER, || {
        client().adjust_allowance(&SPENDER, 970, false)
    })
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
    call(&env, &OWNER, || client().clear_allowance(&SPENDER)).assert_success();
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
        client().adjust_allowance(&SPENDER, i128::MAX - 50, false);
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
        client().adjust_allowance(&SPENDER, i128::MAX, false)
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
            client().adjust_allowance(&SPENDER, difference, false)
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
fn exclusive_zero_delta_conflicts_with_grants_and_spends_in_both_orders() {
    for exclusive_first in [false, true] {
        for grant in [false, true] {
            let env = setup();
            let exclusive = || {
                call(&env, &OWNER, || {
                    client().adjust_allowance(&SPENDER, 0, true)
                })
            };
            let change = || {
                if grant {
                    call(&env, &OWNER, || {
                        client().adjust_allowance(&SPENDER, 10, false)
                    })
                } else {
                    call(&env, &SPENDER, || {
                        client().transfer_from(&OWNER, &OTHER, 10)
                    })
                }
            };
            let (first, second) = if exclusive_first {
                (exclusive(), change())
            } else {
                (change(), exclusive())
            };
            first.assert_success();
            second.assert_commit_failed_nondeterministically();
            env.advance_block();
            call(&env, &OWNER, || {
                assert_eq!(
                    client().allowance(&OWNER, &SPENDER),
                    if exclusive_first {
                        50
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
fn clearing_conflicts_with_concurrent_grants_and_spends_in_both_orders() {
    for clear_first in [false, true] {
        for grant in [false, true] {
            let env = setup();
            let clear =
                || call(&env, &OWNER, || client().clear_allowance(&SPENDER));
            let change = || {
                if grant {
                    call(&env, &OWNER, || {
                        client().adjust_allowance(&SPENDER, 10, false)
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
    call(&env, &OWNER, || client().clear_allowance(&SPENDER)).assert_success();
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
        client().adjust_allowance(&SPENDER, 10, true);
        sdk::call::try_call_raw_with::<(), (), _>(
            &SPENDER,
            entrypoint,
            u64::MAX,
            &(),
            0,
            |error| panic!("spender failed: {error:?}"),
        )
        .unwrap();
        client().clear_allowance(&SPENDER);
        assert_eq!(client().allowance(&OWNER, &SPENDER), 0);
    });
    result.assert_success();
    result.assert_event_count(3);
    result.assert_raw_event(2, |raw| {
        let event = groundhog_erc20_if::AllowanceChangeEvent::try_from(
            EmittedEvent::from(raw.raw()),
        )
        .unwrap();
        assert_eq!(event.difference, -3);
    });
    env.advance_block();
    call(&env, &OWNER, || {
        assert_eq!(client().get_balance(), 93);
        assert_eq!(client().get_balance_of(&OTHER), 7);
        assert_eq!(client().allowance(&OWNER, &SPENDER), 0);
        client().clear_allowance(&SPENDER);
        client().clear_allowance(&SPENDER);
    })
    .assert_success();
}

// These keys deliberately follow the reference contract's storage slots so
// deletion is checked against committed storage, not just its zero-defaulting getter.
fn storage_key(
    env: &TestEnvironmentHandle,
    field: u32,
    account: &ContractAddress,
    spender: Option<&ContractAddress>,
) -> StorageKey {
    let result = env.transact(|tx| {
        tx.call(&OTHER).run(|| {
            use sdk::storage::{Keypath, StorageDomain};
            let path = Keypath::root(TokenContract::DOMAIN)
                .child(&field)
                .child(account);
            match spender {
                Some(spender) => path.child(spender).into_key(),
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
    let allowance_key = storage_key(&env, 2, &OWNER, Some(&SPENDER));
    let balance_key = storage_key(&env, 1, &OWNER, None);
    assert!(
        env.get_snapshot_storage_item(&TOKEN, &allowance_key)
            .is_some()
    );
    assert!(
        env.get_snapshot_storage_item(&TOKEN, &balance_key)
            .is_some()
    );
    let rejected = call(&env, &OWNER, || client().clear_balance());
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
        client().clear_allowance(&SPENDER);
        client().clear_balance();
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
        client().clear_allowance(&SPENDER);
        client().clear_balance();
        assert_eq!(client().get_balance(), 0);
        assert_eq!(client().allowance(&OWNER, &SPENDER), 0);
        assert_eq!(client().total_supply(), 100);
    })
    .assert_success();
}

#[test]
fn clearing_zero_balance_conflicts_with_concurrent_credit_in_both_orders() {
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
            let clear = || call(&env, &OTHER, || client().clear_balance());
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
        let clear = || call(&env, &OWNER, || client().clear_allowance(&OTHER));
        let grant = || {
            call(&env, &OWNER, || {
                client().adjust_allowance(&OTHER, 10, false)
            })
        };
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
            client().adjust_allowance(&SPENDER, difference, false)
        })
        .assert_success();
        call(&env, &OWNER, || {
            client().adjust_allowance(&SPENDER, difference, false)
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
fn clearing_emits_the_removed_amount_and_allows_regrant_in_one_transaction() {
    let env = setup();
    let result = call(&env, &OWNER, || {
        client().clear_allowance(&SPENDER);
        client().clear_allowance(&SPENDER);
        client().adjust_allowance(&SPENDER, 12, false);
        assert_eq!(client().allowance(&OWNER, &SPENDER), 12);
    });
    result.assert_success();
    result.assert_event_count(3);
    for (index, difference) in [-50, 0, 12].into_iter().enumerate() {
        result.assert_raw_event(index, |raw| {
            let event = groundhog_erc20_if::AllowanceChangeEvent::try_from(
                EmittedEvent::from(raw.raw()),
            )
            .unwrap();
            assert_eq!(event.owner, &OWNER);
            assert_eq!(event.spender, &SPENDER);
            assert_eq!(event.difference, difference);
        });
    }
    env.advance_block();
    call(&env, &OWNER, || {
        assert_eq!(client().allowance(&OWNER, &SPENDER), 12);
    })
    .assert_success();
}

#[test]
fn exclusivity_survives_later_nonexclusive_adjustment() {
    for exclusive_first in [true, false] {
        let env = setup();
        let exclusive = || {
            call(&env, &OWNER, || {
                client().adjust_allowance(&SPENDER, 0, true);
                client().adjust_allowance(&SPENDER, 10, false);
            })
        };
        let shared = || {
            call(&env, &SPENDER, || {
                client().transfer_from(&OWNER, &OTHER, 10)
            })
        };
        let (first, second) = if exclusive_first {
            (exclusive(), shared())
        } else {
            (shared(), exclusive())
        };
        first.assert_success();
        second.assert_commit_failed_nondeterministically();
    }
}
#[test]
fn clear_exclusivity_survives_regrant() {
    for clear_first in [true, false] {
        let env = setup();
        let clear = || {
            call(&env, &OWNER, || {
                client().clear_allowance(&OTHER);
                client().adjust_allowance(&OTHER, 50, false);
            })
        };
        let shared = || {
            call(&env, &OWNER, || {
                client().adjust_allowance(&OTHER, 10, false)
            })
        };
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
fn balance_clear_exclusivity_survives_later_credit() {
    for clear_first in [true, false] {
        let env = setup();
        call(&env, &OWNER, || {
            client().adjust_allowance(&OTHER, 10, false)
        })
        .assert_success();
        env.advance_block();
        let clear = || {
            call(&env, &OTHER, || {
                client().clear_balance();
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
