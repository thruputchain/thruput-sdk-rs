//! Test the high-level Balance API over the NNInt storage representation.

use sdk::Named;
use sdk::contract_test;
use sdk::exn::LibraryExn;
use sdk::storage::{Balance, Keypath, NNInt};
use sdk::u127;

contract_test!(balance_defaults_to_zero {
    let balance: Balance = Keypath::default().into();
    assert_eq!(balance.get(), 0);
    assert_eq!(balance.try_get(), Ok(0));
});

contract_test!(balance_credits_and_debits {
    let mut balance: Balance = Keypath::default().into();
    balance.credit(100.into());
    balance.debit(35.into());
    assert_eq!(balance.get(), 65);
});

contract_test!(balance_underflow_is_immediate_and_non_mutating {
    let mut balance: Balance = Keypath::default().into();
    balance.credit(10.into());
    assert_eq!(
        balance.try_debit(11.into()),
        Err(LibraryExn::NonNegativeIntUnderflow)
    );
    assert_eq!(balance.get(), 10);
});

contract_test!(balance_checked_credit_overflow_is_immediate_and_non_mutating {
    let mut balance: Balance = Keypath::default().into();
    balance.checked_credit(u127::max());
    assert_eq!(
        balance.try_checked_credit(1.into()),
        Err(LibraryExn::NonNegativeIntOverflow)
    );
    assert_eq!(balance.get(), i128::MAX as u128);
});

contract_test!(zero_credit_and_debit_leave_missing_balance_unwritten {
    let keypath = Keypath::default();
    let mut balance: Balance = keypath.clone().into();
    balance.credit(0.into());
    balance.debit(0.into());

    let raw: NNInt = keypath.into();
    assert!(raw.try_get().unwrap_err().is_storage_missing());
});

contract_test!(balance_rejects_negative_raw_state {
    let keypath = Keypath::default();
    {
        let mut raw: NNInt = keypath.clone().into();
        raw.set(-1);
    }
    let balance: Balance = keypath.into();
    assert_eq!(
        balance.try_get(),
        Err(LibraryExn::NonNegativeIntUnderflow)
    );
});

#[test]
fn balance_and_nnint_share_committed_storage_representation() {
    let env = sdk::TestEnvironment::no_cost();
    let address = sdk::ContractAddress::named("balance_nnint_compatibility");

    env.transact(|tx| {
        tx.call(&address).run(|| {
            let mut balance: Balance = Keypath::default().into();
            balance.credit(12.into());
        })
    })
    .assert_success();
    env.advance_block();

    env.transact(|tx| {
        tx.call(&address).run(|| {
            let mut raw: NNInt = Keypath::default().into();
            assert_eq!(raw.get(), 12);
            raw.inc(3);
        })
    })
    .assert_success();
    env.advance_block();

    env.transact(|tx| {
        tx.call(&address).run(|| {
            let balance: Balance = Keypath::default().into();
            assert_eq!(balance.get(), 15);
        })
    })
    .assert_success();
}

#[test]
fn lazy_credit_defers_committed_base_overflow() {
    let env = sdk::TestEnvironment::no_cost();
    let address = sdk::ContractAddress::named("balance_lazy_credit");

    env.transact(|tx| {
        tx.call(&address).run(|| {
            let mut balance: Balance = Keypath::default().into();
            balance.checked_credit(u127::max());
        })
    })
    .assert_success();
    env.advance_block();

    let lazy_result = env.transact(|tx| {
        tx.call(&address).run(|| {
            let mut balance: Balance = Keypath::default().into();
            assert_eq!(balance.try_credit(1.into()), Ok(()));
        })
    });
    lazy_result.assert_commit_failed();

    env.transact(|tx| {
        tx.call(&address).run(|| {
            let mut balance: Balance = Keypath::default().into();
            assert_eq!(
                balance.try_checked_credit(1.into()),
                Err(LibraryExn::NonNegativeIntOverflow)
            );
        })
    })
    .assert_success();
}
