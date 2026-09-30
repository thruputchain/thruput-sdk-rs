//! SEP-41 as client code over the issuer token's native interfaces.
use erc20_issuer::*;
use groundhog_sep41::{SEP41Error, SEP41View};
use sdk::*;

const TOKEN: ContractAddress = ContractAddress::new([0x20; 32]);
const ADMIN: ContractAddress = ContractAddress::new([0x21; 32]);
const ALICE: ContractAddress = ContractAddress::new([0x22; 32]);
const BOB: ContractAddress = ContractAddress::new([0x23; 32]);
const SPENDER: ContractAddress = ContractAddress::new([0x24; 32]);
fn view() -> SEP41View<'static> {
    SEP41View::new(&TOKEN)
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
    let contract =
        if let Some(path) = std::env::var_os("ISSUER_TOKEN_RV32_ARTIFACT") {
            sdk::types::Contract::kRv32 {
                rv32_contract: sdk::types::contract::ContractBlob {
                    data: sdk::xdr::BytesM(
                        std::fs::read(path).expect("read RV32 artifact"),
                    ),
                },
            }
        } else {
            erc20_issuer::register_mock_contract(&env).as_contract()
        };
    env.setup_create_contract(&contract);
    env.setup_create_deployment(&TOKEN, &contract);
    call(&env, &ADMIN, || {
        ERC20AdminClient::new(&TOKEN).initialize(
            &ADMIN,
            "Reference Token".try_into().unwrap(),
            "ABCDEFGHIJKL".try_into().unwrap(),
            7,
            Policy {
                auth_required: false,
                auth_revocable: false,
                clawback_enabled: false,
            },
        );
        ERC20Client::new(&TOKEN).mint(&ALICE, 100);
    })
    .assert_success();
    env.advance_block();
    env
}
/// The current height, read from inside a frame.
fn current_ledger(env: &TestEnvironmentHandle) -> i64 {
    let result = env.transact(|tx| tx.call(&BOB).run(get_block_height));
    result.assert_success();
    *result.assert_return_as::<BlockHeight>()
}
fn approve(env: &TestEnvironmentHandle, amount: i128, live_until: i64) {
    call(env, &ALICE, move || {
        view().approve(&SPENDER, amount, Some(live_until.into()))
    })
    .assert_success();
    env.advance_block();
}
fn assert_allowance(env: &TestEnvironmentHandle, expected: i128) {
    call(env, &BOB, move || {
        assert_eq!(view().allowance(&ALICE, &SPENDER), expected)
    })
    .assert_success();
}

#[test]
fn operations_map_onto_the_native_interfaces() {
    let env = setup();
    let ledger = current_ledger(&env);
    call(&env, &ALICE, move || {
        view().transfer(&BOB, 10);
        view().approve(&SPENDER, 30, Some((ledger + 10).into()));
        view().burn(10);
    })
    .assert_success();
    env.advance_block();
    call(&env, &SPENDER, || {
        view().transfer_from(&ALICE, &BOB, 5);
        view().burn_from(&ALICE, 5);
    })
    .assert_success();
    env.advance_block();
    call(&env, &BOB, || {
        assert_eq!(view().balance(&ALICE), 70);
        assert_eq!(view().balance(&BOB), 15);
        assert_eq!(view().allowance(&ALICE, &SPENDER), 20);
        assert_eq!(view().decimals(), 7);
        assert_eq!(view().name().as_str(), "Reference Token");
        assert_eq!(view().symbol().as_str(), "ABCDEFGHIJKL");
        assert_eq!(ERC20Client::new(&TOKEN).total_supply(), 85);
    })
    .assert_success();
}

#[test]
fn approve_replaces_the_allowance_and_zero_revokes_whatever_the_expiry() {
    let env = setup();
    let ledger = current_ledger(&env);
    approve(&env, 30, ledger + 10);
    approve(&env, 10, ledger + 10);
    assert_allowance(&env, 10);
    approve(&env, 0, 0);
    assert_allowance(&env, 0);
}

#[test]
fn approval_is_spendable_through_live_until_ledger_inclusive() {
    let env = setup();
    let ledger = current_ledger(&env);
    approve(&env, 20, ledger + 1);
    call(&env, &SPENDER, || view().transfer_from(&ALICE, &BOB, 5))
        .assert_success();
    env.advance_block();
    assert_allowance(&env, 0);
    call(&env, &SPENDER, || {
        assert_eq!(
            ERC20Client::new(&TOKEN)
                .try_transfer_from(&ALICE, &BOB, 1)
                .unwrap_err()
                .try_as(),
            Some(ERC20Error::TooLarge)
        );
    })
    .assert_success();
}

#[test]
fn approve_rejects_negative_amounts_and_expired_approvals() {
    let env = setup();
    let ledger = current_ledger(&env);
    assert!(ledger > 0);
    for (amount, live_until, expected) in [
        (-1, ledger, SEP41Error::NegativeAmount),
        (1, ledger - 1, SEP41Error::InvalidExpiration),
    ] {
        let result = call(&env, &ALICE, move || {
            view().approve(&SPENDER, amount, Some(live_until.into()))
        });
        assert_eq!(result.assert_abort_as::<SEP41Error>(), expected);
        result.assert_no_events();
    }
}

#[test]
fn approve_conflicts_with_a_same_block_spend_in_both_orders() {
    for approve_first in [false, true] {
        let env = setup();
        let ledger = current_ledger(&env);
        approve(&env, 50, ledger + 10);
        let replace = || {
            call(&env, &ALICE, move || {
                view().approve(&SPENDER, 5, Some((ledger + 10).into()))
            })
        };
        let spend =
            || call(&env, &SPENDER, || view().transfer_from(&ALICE, &BOB, 10));
        let (first, second) = if approve_first {
            (replace(), spend())
        } else {
            (spend(), replace())
        };
        first.assert_success();
        second.assert_commit_failed_nondeterministically();
        env.advance_block();
        assert_allowance(&env, if approve_first { 5 } else { 40 });
    }
}

#[test]
fn is_supported_reads_the_tokens_erc165_claims() {
    const NFT: ContractAddress = ContractAddress::new([0x25; 32]);
    const BARE: ContractAddress = ContractAddress::new([0x26; 32]);
    let env = setup();
    for (addr, contract) in [
        (NFT, erc721::register_mock_contract(&env).as_contract()),
        (BARE, counter::register_mock_contract(&env).as_contract()),
    ] {
        env.setup_create_contract(&contract);
        env.setup_create_deployment(&addr, &contract);
    }
    call(&env, &BOB, || {
        assert_eq!(view().is_supported(), Some(true));
        assert_eq!(SEP41View::new(&NFT).is_supported(), Some(false));
        assert_eq!(SEP41View::new(&BARE).is_supported(), None);
        assert_eq!(view().supports_expiry(), Some(true));
        assert_eq!(SEP41View::new(&NFT).supports_expiry(), Some(false));
        assert_eq!(SEP41View::new(&BARE).supports_expiry(), None);
    })
    .assert_success();
}

#[test]
fn approval_without_a_deadline_never_expires() {
    let env = setup();
    call(&env, &ALICE, || view().approve(&SPENDER, 30, None)).assert_success();
    for _ in 0..3 {
        env.advance_block();
    }
    call(&env, &SPENDER, || {
        view().transfer_from(&ALICE, &BOB, 10);
        assert_eq!(view().allowance(&ALICE, &SPENDER), 20);
    })
    .assert_success();
}

#[test]
fn zero_amount_burn_from_succeeds_after_expiry() {
    let env = setup();
    let ledger = current_ledger(&env);
    approve(&env, 20, ledger);
    assert_allowance(&env, 0);
    call(&env, &SPENDER, || {
        view().burn_from(&ALICE, 0);
        view().transfer_from(&ALICE, &BOB, 0);
    })
    .assert_success();
}

/// The event shapes a SEP-41 postprocessor rebuilds `approve` from: a reset or
/// delete zeroes the allowance, and the change after it is the new amount.
#[test]
fn approve_emits_a_zeroing_event_then_the_amount() {
    let env = setup();
    let ledger = current_ledger(&env);
    let result = call(&env, &ALICE, move || {
        view().approve(&SPENDER, 30, Some((ledger + 10).into()));
        view().approve(&SPENDER, 20, None);
    });
    result.assert_success();
    result.assert_event_count(4);
    result.assert_raw_event(0, |raw| {
        let event =
            AllowanceResetEvent::try_from(EmittedEvent::from(raw.raw()))
                .unwrap();
        assert_eq!((event.removed, *event.until), (0, ledger + 10));
    });
    result.assert_raw_event(2, |raw| {
        let event =
            AllowanceDeleteEvent::try_from(EmittedEvent::from(raw.raw()))
                .unwrap();
        assert_eq!(event.removed, 30);
    });
    for (index, amount) in [(1, 30), (3, 20)] {
        result.assert_raw_event(index, |raw| {
            let event =
                AllowanceChangeEvent::try_from(EmittedEvent::from(raw.raw()))
                    .unwrap();
            assert_eq!(event.difference, amount);
        });
    }
}
