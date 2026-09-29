use groundhog_erc20_if::ERC20Client;
use groundhog_erc20_metadata_if::ERC20MetadataClient;
use groundhog_erc20_permit_if::{
    ClearAllowanceGrant, ERC20PermitClient, PermitGrant,
};
use groundhog_sep41_if::{SEP41ApprovalEvent, SEP41Client, SEP41Error};
use sdk::*;
use sep41_issuer::*;

const TOKEN: ContractAddress = ContractAddress::new([0x30; 32]);
const ADMIN: ContractAddress = ContractAddress::new([0x31; 32]);
const ALICE: ContractAddress = ContractAddress::new([0x32; 32]);
const BOB: ContractAddress = ContractAddress::new([0x33; 32]);
const SPENDER: ContractAddress = ContractAddress::new([0x34; 32]);
fn sep() -> SEP41Client {
    SEP41Client::new(&TOKEN)
}
fn issuer() -> IssuerClient {
    IssuerClient::new(&TOKEN)
}
fn call(
    env: &TestEnvironmentHandle,
    caller: &ContractAddress,
    f: impl FnOnce() + std::panic::UnwindSafe,
) -> TransactResult {
    env.transact(|tx| tx.call(caller).run(f))
}
fn setup() -> TestEnvironmentHandle {
    let env = TestEnvironment::no_cost();
    if let Some(path) = std::env::var_os("SEP41_TOKEN_RV32_ARTIFACT") {
        let contract = sdk::types::Contract::kRv32 {
            rv32_contract: sdk::types::contract::ContractBlob {
                data: sdk::xdr::BytesM(
                    std::fs::read(path).expect("read RV32 artifact"),
                ),
            },
        };
        env.setup_create_contract(&contract);
        env.setup_create_deployment(&TOKEN, &contract);
    } else {
        sep41_issuer::register_mock_contract(&env).deploy_contract_to(&TOKEN);
    }
    call(&env, &ADMIN, || {
        ERC20AdminClient::new(&TOKEN).initialize(
            &ADMIN,
            "Reference Token".try_into().unwrap(),
            "TEST".try_into().unwrap(),
            7,
            Policy {
                auth_required: false,
                auth_revocable: true,
                clawback_enabled: true,
            },
        );
        issuer().mint(&ALICE, 100);
    })
    .assert_success();
    env.advance_block();
    env
}
fn height(env: &TestEnvironmentHandle) -> u32 {
    let result =
        env.transact(|tx| tx.call(&ALICE).run(sdk::block::get_block_height));
    result.assert_success();
    result
        .assert_return_as::<BlockHeight>()
        .into_inner()
        .try_into()
        .unwrap()
}
fn approve(env: &TestEnvironmentHandle, amount: i128, expiry: u32) {
    call(env, &ALICE, || {
        sep().approve(&ALICE, &SPENDER, amount, expiry)
    })
    .assert_success();
    env.advance_block();
}
fn state(
    env: &TestEnvironmentHandle,
    alice: i128,
    bob: i128,
    allowance: i128,
) {
    call(env, &BOB, || {
        assert_eq!(sep().balance(&ALICE), alice);
        assert_eq!(sep().balance(&BOB), bob);
        assert_eq!(sep().allowance(&ALICE, &SPENDER), allowance);
        assert_eq!(issuer().total_supply(), alice + bob);
    })
    .assert_success();
}
fn sep_error(result: TransactResult, expected: SEP41Error) {
    assert_eq!(result.assert_abort_as::<SEP41Error>(), expected);
    result.assert_no_events();
}
fn erc_error(result: TransactResult, expected: ERC20Error) {
    assert_eq!(result.assert_abort_as::<ERC20Error>(), expected);
    result.assert_no_events();
}

#[test]
fn metadata_and_balances_use_the_initialized_sep41_token() {
    let env = setup();
    call(&env, &BOB, || {
        assert_eq!(sep().name().as_str(), "Reference Token");
        assert_eq!(sep().symbol().as_str(), "TEST");
        assert_eq!(sep().decimals(), 7);
        assert_eq!(sep().balance(&BOB), 0);
        assert_eq!(sep().allowance(&ALICE, &SPENDER), 0);
    })
    .assert_success();
    call(&env, &ALICE, || sep().transfer(&ALICE, &BOB, 12)).assert_success();
    env.advance_block();
    state(&env, 88, 12, 0);
}

#[test]
fn explicit_addresses_cannot_impersonate_the_owner_or_spender() {
    let env = setup();
    approve(&env, 20, u32::MAX);
    for result in [
        call(&env, &BOB, || sep().approve(&ALICE, &SPENDER, 1, u32::MAX)),
        call(&env, &BOB, || sep().transfer(&ALICE, &BOB, 1)),
        call(&env, &BOB, || sep().burn(&ALICE, 1)),
        call(&env, &BOB, || {
            sep().transfer_from(&SPENDER, &ALICE, &BOB, 1)
        }),
        call(&env, &BOB, || sep().burn_from(&SPENDER, &ALICE, 1)),
    ] {
        sep_error(result, SEP41Error::Unauthorized);
    }
    state(&env, 100, 0, 20);
}

#[test]
fn every_amount_argument_rejects_negative_values() {
    let env = setup();
    approve(&env, 20, u32::MAX);
    for amount in [-1, i128::MIN] {
        for result in [
            call(&env, &ALICE, || {
                sep().approve(&ALICE, &SPENDER, amount, u32::MAX)
            }),
            call(&env, &ALICE, || sep().transfer(&ALICE, &BOB, amount)),
            call(&env, &ALICE, || sep().burn(&ALICE, amount)),
            call(&env, &SPENDER, || {
                sep().transfer_from(&SPENDER, &ALICE, &BOB, amount)
            }),
            call(&env, &SPENDER, || sep().burn_from(&SPENDER, &ALICE, amount)),
        ] {
            sep_error(result, SEP41Error::NegativeAmount);
        }
    }
    state(&env, 100, 0, 20);
}

#[test]
fn approval_replaces_the_remaining_amount_and_emits_one_event() {
    let env = setup();
    approve(&env, 60, u32::MAX);
    call(&env, &SPENDER, || {
        sep().transfer_from(&SPENDER, &ALICE, &BOB, 10)
    })
    .assert_success();
    env.advance_block();
    let expiry = height(&env) + 5;
    let result =
        call(&env, &ALICE, || sep().approve(&ALICE, &SPENDER, 25, expiry));
    result.assert_success();
    result.assert_event_count(1);
    result.assert_raw_event(0, |e| {
        let event =
            SEP41ApprovalEvent::try_from(EmittedEvent::from(e.raw())).unwrap();
        assert_eq!(event.from, &ALICE);
        assert_eq!(event.spender, &SPENDER);
        assert_eq!(event.amount, 25);
        assert_eq!(event.live_until_ledger, expiry);
    });
    env.advance_block();
    state(&env, 90, 10, 25);
}

#[test]
fn expiry_is_inclusive_and_spending_does_not_extend_it() {
    let env = setup();
    let deadline = height(&env) + 1;
    approve(&env, 20, deadline);
    assert_eq!(height(&env), deadline);
    call(&env, &SPENDER, || {
        assert_eq!(sep().allowance(&ALICE, &SPENDER), 20);
        sep().transfer_from(&SPENDER, &ALICE, &BOB, 5);
        assert_eq!(sep().allowance(&ALICE, &SPENDER), 15);
    })
    .assert_success();
    env.advance_block();
    state(&env, 95, 5, 0);
    erc_error(
        call(&env, &SPENDER, || sep().burn_from(&SPENDER, &ALICE, 1)),
        ERC20Error::TooLarge,
    );
}

#[test]
fn past_expiry_rejects_new_grants_but_allows_zero_revocation() {
    let env = setup();
    approve(&env, 20, u32::MAX);
    let past = height(&env) - 1;
    sep_error(
        call(&env, &ALICE, || sep().approve(&ALICE, &SPENDER, 1, past)),
        SEP41Error::InvalidExpiration,
    );
    state(&env, 100, 0, 20);
    call(&env, &ALICE, || sep().approve(&ALICE, &SPENDER, 0, past))
        .assert_success();
    env.advance_block();
    state(&env, 100, 0, 0);
    erc_error(
        call(&env, &SPENDER, || {
            sep().transfer_from(&SPENDER, &ALICE, &BOB, 1)
        }),
        ERC20Error::TooLarge,
    );
}

fn unknown_method(result: TransactResult) {
    assert_eq!(
        result.assert_abort_as::<sdk::exn::LibraryExn>(),
        sdk::exn::LibraryExn::UnknownMethod
    );
    result.assert_no_events();
}

#[test]
fn native_entrypoints_cannot_grant_spend_or_modify_the_sep41_token() {
    let env = setup();
    approve(&env, 25, u32::MAX);
    let native = ERC20Client::new(&TOKEN);
    let metadata = ERC20MetadataClient::new(&TOKEN);
    let burn = ERC20BurnClient::new(&TOKEN);
    for result in [
        call(&env, &ALICE, || {
            native.adjust_allowance(&SPENDER, 40, false)
        }),
        call(&env, &ALICE, || native.clear_allowance(&SPENDER)),
        call(&env, &SPENDER, || native.transfer_from(&ALICE, &BOB, 10)),
        call(&env, &ALICE, || native.transfer(&BOB, 10)),
        call(&env, &ALICE, || native.clear_balance()),
        call(&env, &BOB, || {
            native.allowance(&ALICE, &SPENDER);
        }),
        call(&env, &BOB, || {
            native.get_balance_of(&ALICE);
        }),
        call(&env, &ALICE, || {
            native.get_balance();
        }),
        call(&env, &ADMIN, || native.initialize(&ADMIN)),
        call(&env, &ADMIN, || native.mint(&ALICE, 1)),
        call(&env, &ADMIN, || native.burn(&ALICE, 1)),
        call(&env, &BOB, || {
            native.total_supply();
        }),
        call(&env, &ALICE, || burn.burn(1)),
        call(&env, &SPENDER, || burn.burn_from(&ALICE, 1)),
        call(&env, &ADMIN, || {
            metadata.initialize(&ADMIN, None, None, None)
        }),
        call(&env, &BOB, || {
            metadata.name();
        }),
        call(&env, &BOB, || {
            metadata.symbol();
        }),
        call(&env, &BOB, || {
            metadata.decimals();
        }),
    ] {
        unknown_method(result);
    }
    env.advance_block();
    state(&env, 100, 0, 25);
    call(&env, &SPENDER, || {
        sep().transfer_from(&SPENDER, &ALICE, &BOB, 5)
    })
    .assert_success();
    env.advance_block();
    state(&env, 95, 5, 20);
}

#[test]
fn native_permit_entrypoints_cannot_bypass_expiring_approval() {
    use sdk::mintable::{AuthTokenPreimage, MintableType};
    use sdk::signer::mint_auth_token;
    type GrantData = <PermitGrant as MintableType>::Preimage;
    type ClearData = <ClearAllowanceGrant as MintableType>::Preimage;
    let env = setup();
    let expiry = height(&env) + 1;
    approve(&env, 25, expiry);
    env.advance_block();
    state(&env, 100, 0, 0);
    for increase in [false, true] {
        unknown_method(call(&env, &ALICE, || {
            let grant = mint_auth_token::<PermitGrant>(
                sdk::block::get_block_height(),
                AuthTokenPreimage {
                    consumer: TOKEN,
                    data: GrantData {
                        spender: ALICE,
                        amount: 40,
                    },
                },
            );
            let permit = ERC20PermitClient::new(&TOKEN);
            if increase {
                permit.increase_allowance_signed(grant);
            } else {
                permit.transfer_from_signed(grant, &BOB, 5);
            }
        }));
    }
    unknown_method(call(&env, &ALICE, || {
        let grant = mint_auth_token::<ClearAllowanceGrant>(
            sdk::block::get_block_height(),
            AuthTokenPreimage {
                consumer: TOKEN,
                data: ClearData { spender: SPENDER },
            },
        );
        ERC20PermitClient::new(&TOKEN).clear_allowance_signed(grant);
    }));
    env.advance_block();
    state(&env, 100, 0, 0);
    erc_error(
        call(&env, &SPENDER, || {
            sep().transfer_from(&SPENDER, &ALICE, &BOB, 1)
        }),
        ERC20Error::TooLarge,
    );
}

#[test]
fn transfer_and_burn_emit_only_the_canonical_movement_events() {
    let env = setup();
    approve(&env, 30, u32::MAX);
    let result = call(&env, &SPENDER, || {
        sep().transfer_from(&SPENDER, &ALICE, &BOB, 10);
        sep().burn_from(&SPENDER, &ALICE, 5);
    });
    result.assert_success();
    result.assert_event_count(2);
    result.assert_raw_event(0, |e| {
        let event =
            TransferEvent::try_from(EmittedEvent::from(e.raw())).unwrap();
        assert_eq!(event.from, &ALICE);
        assert_eq!(event.to, &BOB);
        assert_eq!(event.amount, 10);
    });
    result.assert_raw_event(1, |e| {
        let event = BurnEvent::try_from(EmittedEvent::from(e.raw())).unwrap();
        assert_eq!(event.amount, 5);
    });
    env.advance_block();
    state(&env, 85, 10, 15);
    call(&env, &BOB, || assert_eq!(issuer().total_supply(), 95))
        .assert_success();
}

#[test]
fn frozen_holders_can_revoke_but_cannot_move_or_burn() {
    let env = setup();
    approve(&env, 20, u32::MAX);
    call(&env, &ADMIN, || {
        ERC20AuthorizationClient::new(&TOKEN).set_authorized(&ALICE, false)
    })
    .assert_success();
    env.advance_block();
    for result in [
        call(&env, &ALICE, || sep().transfer(&ALICE, &BOB, 1)),
        call(&env, &ALICE, || sep().burn(&ALICE, 1)),
        call(&env, &SPENDER, || {
            sep().transfer_from(&SPENDER, &ALICE, &BOB, 1)
        }),
        call(&env, &SPENDER, || sep().burn_from(&SPENDER, &ALICE, 1)),
    ] {
        assert_eq!(
            result.assert_abort_as::<ERC20ControlsError>(),
            ERC20ControlsError::HolderNotAuthorized
        );
        result.assert_no_events();
    }
    state(&env, 100, 0, 20);
    call(&env, &ALICE, || sep().approve(&ALICE, &SPENDER, 0, 0))
        .assert_success();
    env.advance_block();
    call(&env, &ADMIN, || {
        ERC20ClawbackClient::new(&TOKEN).clawback(&ALICE, 5)
    })
    .assert_success();
    env.advance_block();
    state(&env, 95, 0, 0);
}

#[test]
fn caught_recipient_policy_failure_restores_allowance() {
    let env = setup();
    approve(&env, 20, u32::MAX);
    call(&env, &ADMIN, || {
        ERC20AuthorizationClient::new(&TOKEN).set_authorized(&BOB, false)
    })
    .assert_success();
    env.advance_block();
    let result = call(&env, &SPENDER, || {
        assert_eq!(
            sep()
                .try_transfer_from(&SPENDER, &ALICE, &BOB, 5)
                .unwrap_err()
                .try_as(),
            Some(ERC20ControlsError::HolderNotAuthorized)
        );
        assert_eq!(sep().allowance(&ALICE, &SPENDER), 20);
    });
    result.assert_success();
    result.assert_no_events();
    env.advance_block();
    state(&env, 100, 0, 20);
}

#[test]
fn same_transaction_replacement_spending_and_replacement_keep_exclusivity() {
    let env = setup();
    call(&env, &ALICE, || sep().approve(&ALICE, &ALICE, 50, u32::MAX))
        .assert_success();
    env.advance_block();
    call(&env, &ALICE, || {
        sep().approve(&ALICE, &ALICE, 30, u32::MAX);
        sep().transfer_from(&ALICE, &ALICE, &BOB, 10);
        sep().approve(&ALICE, &ALICE, 7, u32::MAX);
        assert_eq!(sep().allowance(&ALICE, &ALICE), 7);
    })
    .assert_success();
    call(&env, &ALICE, || {
        sep().transfer_from(&ALICE, &ALICE, &BOB, 5)
    })
    .assert_commit_failed_nondeterministically();
    env.advance_block();
    call(&env, &ALICE, || {
        assert_eq!(sep().allowance(&ALICE, &ALICE), 7)
    })
    .assert_success();
    state(&env, 90, 10, 0);
}

#[test]
fn replacement_and_spending_conflict_in_both_commit_orders() {
    for replacement_first in [false, true] {
        let env = setup();
        approve(&env, 50, u32::MAX);
        let replace = || {
            call(&env, &ALICE, || {
                sep().approve(&ALICE, &SPENDER, 20, u32::MAX)
            })
        };
        let spend = || {
            call(&env, &SPENDER, || {
                sep().transfer_from(&SPENDER, &ALICE, &BOB, 10)
            })
        };
        let (first, second) = if replacement_first {
            (replace(), spend())
        } else {
            (spend(), replace())
        };
        first.assert_success();
        second.assert_commit_failed_nondeterministically();
        env.advance_block();
        if replacement_first {
            state(&env, 100, 0, 20);
        } else {
            state(&env, 90, 10, 40);
        }
    }
}

#[test]
fn ordinary_spends_merge_until_the_combined_allowance_is_exhausted() {
    let env = setup();
    approve(&env, 25, u32::MAX);
    for _ in 0..2 {
        call(&env, &SPENDER, || {
            sep().transfer_from(&SPENDER, &ALICE, &BOB, 10)
        })
        .assert_success();
    }
    call(&env, &SPENDER, || sep().burn_from(&SPENDER, &ALICE, 10))
        .assert_commit_failed_nondeterministically();
    env.advance_block();
    state(&env, 80, 20, 5);
}

#[test]
fn maximum_signed_grant_and_self_transfer_preserve_accounting() {
    let env = setup();
    approve(&env, i128::MAX, u32::MAX);
    call(&env, &SPENDER, || {
        sep().transfer_from(&SPENDER, &ALICE, &ALICE, 100)
    })
    .assert_success();
    env.advance_block();
    state(&env, 100, 0, i128::MAX - 100);
    erc_error(
        call(&env, &SPENDER, || {
            sep().transfer_from(&SPENDER, &ALICE, &BOB, 101)
        }),
        ERC20Error::TooLarge,
    );
    state(&env, 100, 0, i128::MAX - 100);
}

#[test]
fn zero_spending_without_a_grant_does_not_create_spending_power() {
    let env = setup();
    call(&env, &SPENDER, || {
        sep().transfer_from(&SPENDER, &ALICE, &BOB, 0);
        sep().burn_from(&SPENDER, &ALICE, 0);
        assert_eq!(sep().allowance(&ALICE, &SPENDER), 0);
    })
    .assert_success();
    env.advance_block();
    erc_error(
        call(&env, &SPENDER, || {
            sep().transfer_from(&SPENDER, &ALICE, &BOB, 1)
        }),
        ERC20Error::TooLarge,
    );
    state(&env, 100, 0, 0);
}

#[test]
fn changing_only_expiry_conflicts_with_spending_in_both_commit_orders() {
    for approval_first in [false, true] {
        let env = setup();
        approve(&env, 50, u32::MAX);
        let deadline = height(&env) + 1;
        let change_expiry = || {
            call(&env, &ALICE, || {
                sep().approve(&ALICE, &SPENDER, 50, deadline)
            })
        };
        let spend = || {
            call(&env, &SPENDER, || {
                sep().transfer_from(&SPENDER, &ALICE, &BOB, 10)
            })
        };
        let (first, second) = if approval_first {
            (change_expiry(), spend())
        } else {
            (spend(), change_expiry())
        };
        first.assert_success();
        second.assert_commit_failed_nondeterministically();
        env.advance_block();
        if approval_first {
            state(&env, 100, 0, 50);
        } else {
            state(&env, 90, 10, 40);
        }
        env.advance_block();
        if approval_first {
            state(&env, 100, 0, 0);
        } else {
            state(&env, 90, 10, 40);
        }
    }
}

#[test]
fn regrant_after_expiry_replaces_the_expired_remaining_amount() {
    let env = setup();
    let deadline = height(&env) + 1;
    approve(&env, 50, deadline);
    call(&env, &SPENDER, || {
        sep().transfer_from(&SPENDER, &ALICE, &BOB, 10)
    })
    .assert_success();
    env.advance_block();
    state(&env, 90, 10, 0);
    approve(&env, 7, u32::MAX);
    state(&env, 90, 10, 7);
    call(&env, &SPENDER, || {
        sep().transfer_from(&SPENDER, &ALICE, &BOB, 7)
    })
    .assert_success();
    env.advance_block();
    state(&env, 83, 17, 0);
}

#[test]
fn issuer_mint_checks_authority_holder_policy_and_signed_bounds() {
    let env = setup();
    erc_error(
        call(&env, &BOB, || issuer().mint(&BOB, 1)),
        ERC20Error::MustBeContractOwner,
    );
    for amount in [-1, i128::MIN] {
        sep_error(
            call(&env, &ADMIN, || issuer().mint(&ALICE, amount)),
            SEP41Error::NegativeAmount,
        );
    }
    erc_error(
        call(&env, &ADMIN, || issuer().mint(&ALICE, i128::MAX)),
        ERC20Error::TooLarge,
    );
    call(&env, &ADMIN, || {
        ERC20AuthorizationClient::new(&TOKEN).set_authorized(&BOB, false)
    })
    .assert_success();
    env.advance_block();
    let result = call(&env, &ADMIN, || issuer().mint(&BOB, 1));
    assert_eq!(
        result.assert_abort_as::<ERC20ControlsError>(),
        ERC20ControlsError::HolderNotAuthorized
    );
    result.assert_no_events();
    state(&env, 100, 0, 0);
}
