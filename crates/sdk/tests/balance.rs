//! Test the high-level Balance API over the NNInt storage representation.

use sdk::Named;
use sdk::contract_test;
use sdk::exn::LibraryExn;
use sdk::storage::{Balance, Keypath, NNInt};

contract_test!(balance_defaults_to_zero {
    let balance: Balance = Keypath::default().into();
    assert_eq!(balance.get(), 0);
    assert_eq!(balance.try_get(), Ok(0));
});

contract_test!(balance_increases_and_decreases {
    let mut balance: Balance = Keypath::default().into();
    balance.increase(100);
    balance.decrease(35);
    assert_eq!(balance.get(), 65);
});

contract_test!(balance_lazily_increases_and_decreases {
    let mut balance: Balance = Keypath::default().into();
    balance.lazy_increase(100);
    balance.lazy_decrease(35);
    assert_eq!(balance.get(), 65);
});

contract_test!(balance_underflow_is_immediate_and_non_mutating {
    let mut balance: Balance = Keypath::default().into();
    balance.increase(10);
    assert_eq!(
        balance.try_decrease(11),
        Err(LibraryExn::NonNegativeIntUnderflow)
    );
    assert_eq!(balance.get(), 10);
});

contract_test!(lazy_decrease_checks_a_balance_written_this_transaction {
    // A checked increase leaves a pending "set" value, which the storage
    // layer requires the accumulated delta to respect, so the underflow is
    // reported here rather than deferred to commit.
    let mut balance: Balance = Keypath::default().into();
    balance.increase(10);
    assert_eq!(
        balance.try_lazy_decrease(11),
        Err(LibraryExn::NonNegativeIntUnderflow)
    );
    assert_eq!(balance.get(), 10);
});

contract_test!(balance_increase_overflow_is_immediate_and_non_mutating {
    let mut balance: Balance = Keypath::default().into();
    balance.increase(Balance::MAX);
    assert_eq!(
        balance.try_increase(1),
        Err(LibraryExn::NonNegativeIntOverflow)
    );
    assert_eq!(balance.get(), Balance::MAX);
});

contract_test!(zero_adjustments_leave_missing_balance_unwritten {
    let keypath = Keypath::default();
    let mut balance: Balance = keypath.clone().into();
    balance.increase(0);
    balance.decrease(0);
    balance.lazy_increase(0);
    balance.lazy_decrease(0);

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

contract_test!(balance_rejects_amounts_above_i128_max {
    let mut balance: Balance = Keypath::default().into();
    let too_large = (i128::MAX as u128) + 1;
    assert_eq!(
        balance.try_increase(too_large),
        Err(LibraryExn::NonNegativeIntOverflow)
    );
    assert_eq!(
        balance.try_lazy_increase(too_large),
        Err(LibraryExn::NonNegativeIntOverflow)
    );
    assert_eq!(
        balance.try_decrease(too_large),
        Err(LibraryExn::NonNegativeIntUnderflow)
    );
    assert_eq!(
        balance.try_lazy_decrease(too_large),
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
            balance.increase(12);
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
fn lazy_increase_defers_committed_base_overflow() {
    let env = sdk::TestEnvironment::no_cost();
    let address = sdk::ContractAddress::named("balance_lazy_increase");

    // Set balance to max value
    env.transact(|tx| {
        tx.call(&address).run(|| {
            let mut balance: Balance = Keypath::default().into();
            balance.increase(Balance::MAX);
        })
    })
    .assert_success();

    env.advance_block();

    // Lazy increase tx should succeed but fail to commit
    env.transact(|tx| {
        tx.call(&address).run(|| {
            let mut balance: Balance = Keypath::default().into();
            assert_eq!(balance.try_lazy_increase(1), Ok(()));
        })
    })
    .assert_commit_failed();

    // Checked increase should fail immediately
    env.transact(|tx| {
        tx.call(&address).run(|| {
            let mut balance: Balance = Keypath::default().into();
            assert_eq!(
                balance.try_increase(1),
                Err(LibraryExn::NonNegativeIntOverflow)
            );
        })
    })
    .assert_success();
}

#[test]
fn lazy_increase_accepts_overflow_of_loaded_value() {
    let env = sdk::TestEnvironment::no_cost();
    let address = sdk::ContractAddress::named("balance_lazy_increase");

    // Set balance to max value
    env.transact(|tx| {
        tx.call(&address).run(|| {
            let mut balance: Balance = Keypath::default().into();
            balance.increase(Balance::MAX);
        })
    })
    .assert_success();

    env.advance_block();

    env.transact(|tx| {
        tx.call(&address).run(|| {
            // Load the committed value
            let balance: Balance = Keypath::default().into();
            assert_eq!(balance.get(), Balance::MAX);

            // Lazy increase: accepts overflowing increase despite
            // previously-loaded value
            let mut balance: Balance = Keypath::default().into();
            assert_eq!(balance.try_lazy_increase(1), Ok(()));
        })
    })
    .assert_commit_failed();
}

#[test]
fn lazy_decrease_defers_committed_base_underflow() {
    let env = sdk::TestEnvironment::no_cost();
    let address = sdk::ContractAddress::named("balance_lazy_decrease");

    env.transact(|tx| {
        tx.call(&address).run(|| {
            let mut balance: Balance = Keypath::default().into();
            balance.increase(10);
        })
    })
    .assert_success();

    env.advance_block();

    // Lazy decrease tx should succeed but fail to commit
    env.transact(|tx| {
        tx.call(&address).run(|| {
            let mut balance: Balance = Keypath::default().into();
            assert_eq!(balance.try_lazy_decrease(11), Ok(()));
        })
    })
    .assert_commit_failed();

    // Checked decrease should fail immediately
    env.transact(|tx| {
        tx.call(&address).run(|| {
            let mut balance: Balance = Keypath::default().into();
            assert_eq!(
                balance.try_decrease(11),
                Err(LibraryExn::NonNegativeIntUnderflow)
            );
        })
    })
    .assert_success();
}

#[test]
fn lazy_decrease_accepts_underflow_of_loaded_value() {
    let env = sdk::TestEnvironment::no_cost();
    let address = sdk::ContractAddress::named("balance_lazy_decrease");

    env.transact(|tx| {
        tx.call(&address).run(|| {
            let mut balance: Balance = Keypath::default().into();
            balance.increase(10);
        })
    })
    .assert_success();

    env.advance_block();

    env.transact(|tx| {
        tx.call(&address).run(|| {
            // Load the committed value
            let balance: Balance = Keypath::default().into();
            assert_eq!(balance.get(), 10);

            // Lazy decrease: accepts underflowing decrease despite
            // previously-loaded value
            let mut balance: Balance = Keypath::default().into();
            assert_eq!(balance.try_lazy_decrease(11), Ok(()));
        })
    })
    .assert_commit_failed();
}
