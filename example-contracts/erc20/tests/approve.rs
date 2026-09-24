use erc20::*;
use sdk::*;

const TOKEN: ContractAddress = ContractAddress::new([0x31; 32]);
const OWNER: ContractAddress = ContractAddress::new([0x32; 32]);
const SPENDER: ContractAddress = ContractAddress::new([0x33; 32]);
const OTHER: ContractAddress = ContractAddress::new([0x34; 32]);
// Explicit opt-in: this requires a hogd test FFI containing native approve.
fn native_token() -> bool {
    std::env::var_os("ERC20_APPROVE_NATIVE").is_some()
}
fn client() -> ERC20Client {
    ERC20Client::new(if native_token() {
        &sdk::precompiles::ADDRESS_NATIVE_TOKEN
    } else {
        &TOKEN
    })
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
    if native_token() {
        call(&env, &sdk::precompiles::ADDRESS_GENESIS_ROOT, || {
            client().mint(&OWNER, 100)
        })
        .assert_success();
        env.advance_block();
    } else {
        let mock = erc20::register_mock_contract(&env);
        env.setup_create_deployment(&TOKEN, &mock.as_contract());
        call(&env, &OWNER, || {
            client().initialize(&OWNER);
            client().mint(&OWNER, 100);
        })
        .assert_success();
        env.advance_block();
    }
    call(&env, &OWNER, || {
        client().approve(&SPENDER, 50);
    })
    .assert_success();
    env.advance_block();
    env
}

#[test]
fn approval_conflicts_with_spending_and_legacy_adjustment_in_both_orders() {
    for approval_first in [false, true] {
        for adjustment in [false, true] {
            for replacement in [0, 40, 50] {
                let env = setup();
                let approve = || {
                    call(&env, &OWNER, || {
                        client().approve(&SPENDER, replacement)
                    })
                };
                let change = || {
                    if adjustment {
                        call(&env, &OWNER, || {
                            client().adjust_allowance(&SPENDER, 10)
                        })
                    } else {
                        call(&env, &SPENDER, || {
                            client().transfer_from(&OWNER, &OTHER, 10)
                        })
                    }
                };
                let (first, second) = if approval_first {
                    (approve(), change())
                } else {
                    (change(), approve())
                };
                first.assert_success();
                second.assert_commit_failed_nondeterministically();
                env.advance_block();
                call(&env, &OWNER, || {
                    assert_eq!(
                        client().allowance(&OWNER, &SPENDER),
                        if approval_first {
                            replacement
                        } else if adjustment {
                            60
                        } else {
                            40
                        }
                    );
                    assert_eq!(
                        client().get_balance_of(&OTHER),
                        if !approval_first && !adjustment {
                            10
                        } else {
                            0
                        }
                    );
                })
                .assert_success();
            }
        }
    }
}

#[test]
fn equal_replacements_conflict_but_independent_spenders_can_commit() {
    let env = setup();
    call(&env, &OWNER, || client().approve(&SPENDER, 20)).assert_success();
    call(&env, &OWNER, || client().approve(&SPENDER, 20))
        .assert_commit_failed_nondeterministically();
    call(&env, &OWNER, || client().approve(&OTHER, 30)).assert_success();
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
    call(&env, &OWNER, || client().approve(&SPENDER, 1000)).assert_success();
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
    call(&env, &OWNER, || client().approve(&SPENDER, 0)).assert_success();
    env.advance_block();
    call(&env, &SPENDER, || {
        assert_eq!(client().allowance(&OWNER, &SPENDER), 0);
        assert_eq!(client().get_balance_of(&OTHER), 20);
    })
    .assert_success();
}

#[test]
fn absolute_event_and_amount_bound() {
    use groundhog_erc20_if::{ApprovalEvent, ApproveEvent, ERC20Error};
    let env = setup();
    let result = call(&env, &OWNER, || {
        client().approve(&SPENDER, i128::MAX as u128);
        assert_eq!(
            client()
                .try_approve(&SPENDER, i128::MAX as u128 + 1)
                .unwrap_err()
                .try_as(),
            Some(ERC20Error::TooLarge)
        );
        assert_eq!(client().allowance(&OWNER, &SPENDER), i128::MAX as u128);
    });
    result.assert_success();
    result.assert_event_count(1);
    result.assert_raw_event(0, |event| {
        let event =
            ApproveEvent::try_from(EmittedEvent::from(event.raw())).unwrap();
        assert_eq!(event.owner, &OWNER);
        assert_eq!(event.spender, &SPENDER);
        assert_eq!(event.amount, i128::MAX);
        assert_ne!(ApproveEvent::SIGNATURE, ApprovalEvent::SIGNATURE);
    });
}
