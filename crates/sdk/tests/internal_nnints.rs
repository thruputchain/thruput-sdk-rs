//! Test the semantics of the raw nnint syscalls

#![allow(unused)]

mod common;
use common::*;

use thruput_sdk::call::RawCallError;
use thruput_sdk::exn::{Exn, LibraryExn};
use thruput_sdk::sys::{SyscallErrorCode, SyscallNo};
use thruput_sdk::sys_exn::bytestring::*;
use thruput_sdk::sys_exn::nnint_internal::*;

const SHARED: bool = false;
use thruput_sdk::*;

/// Run `f` as a single transaction at this file's test address.
fn run_tx(f: impl FnOnce() + std::panic::UnwindSafe) -> TransactResult {
    let env = TestEnvironment::no_cost();
    env.transact(|tx| {
        tx.call(&ContractAddress::named("internal_nnints")).run(f)
    })
}

/// Run `f` and decode the root-frame outcome as a client-style result,
/// for matching typed exceptions.
#[allow(clippy::result_large_err)]
fn run_tx_result(
    f: impl FnOnce() + std::panic::UnwindSafe,
) -> Result<(), RawCallError> {
    into_root_call_result(&run_tx(f))
}

#[rustfmt::skip]
contract_test!(test_internal_nnints {
    let key = StorageKey::default();
    let key = &key;

    // Assuming no previous activity at this storage location, these are the values
    // associated with it:
    let mut set = 0;
    let mut add = 0;

        // Get (without forcing) on a previously-unset location returns "add" == zero
        assert_eq!(get_values(key), (None, add));

    // `set_add` (specifying only "add") to a previously-unset location succeeds
    add = 100;
    set_add(key, None, add, SHARED);

        // We get just the "add" value back when not forcing
        assert_eq!(get_values(key), (None, add));

        // We get the "set" and "add" values back when forcing; "set" was restored by the `set_add`
        // above to its state at the start of the transaction
        assert_eq!(get_values_force(key), (set, add));

    // `set_add` (specifying only "add") again succeeds
    add = 3;
    set_add(key, None, add, SHARED);

        // The "add" value is now what we provided in the last call, NOT a running sum
        assert_eq!(get_values_force(key), (set, add));

        // We still get just the "add" value back when not forcing, even though it
        // has been forced (but not set) before
        assert_eq!(get_values(key), (None, add));

    // We now provide a non-zero "set" value, and clear "add"
    set = 7;
    add = 0;
    set_add(key, Some(set), add, SHARED);

        // Even if we don't force, we'll see the transaction's latest "set" value
        assert_eq!(get_values(key), (Some(set), add));

        // Same happens if we do force
        assert_eq!(get_values_force(key), (set, add));

    // Here we just specify "add" again ...
    add = 17;
    set_add(key, None, add, SHARED);

        // The unspecified "set" value has been restored to its value at the start of the
        // transaction
        set = 0;
        assert_eq!(get_values_force(key), (set, add));

    // We can also provide "set" and "add" at the same time, for whatever that's worth
    set = 8;
    add = 2;
    set_add(key, Some(set), add, SHARED);

        // They've both been updated
        assert_eq!(get_values_force(key), (set, add));

    // TODO: test with `delete`
});

#[test]
// Get (without forcing) on a never-before-set location returns "add" == zero
fn get_on_empty_succeeds() {
    run_tx(|| {
        get_values(&StorageKey::default());
    })
    .assert_success();
}

#[test]
// Get (not forcing) on a wrong-type location succeeds.
fn get_on_wrong_type_succeeds() {
    run_tx(|| {
        let key = StorageKey::default();

        bytestring_set(&key, b"");
        get_values(&key);
    })
    .assert_success();
}

#[test]
// Get (forcing) on a never-before-set location fails with not-exist
fn force_get_on_empty_fails() {
    let res = run_tx_result(|| {
        get_values_force(&StorageKey::default());
    });

    assert_syscall_exception(
        &res,
        SyscallNo::kNonnegativeIntGet,
        SyscallErrorCode::kSyscallErrorCodeStorageKeyNotExist,
    );
}

#[test]
// Get (forcing) on a wrong-type location fails with invalid-input
fn force_get_on_wrong_type_fails() {
    let res = run_tx_result(|| {
        let key = StorageKey::default();

        bytestring_set(&key, b"");
        get_values_force(&key);
    });

    assert_syscall_exception(
        &res,
        SyscallNo::kNonnegativeIntGet,
        SyscallErrorCode::kSyscallErrorCodeInvalidSyscallInput,
    );
}

#[test]
// We can't give "set" and "add" values whose sum is negative.
fn set_add_negative_sum_fails() {
    let res = run_tx_result(|| {
        let set = 1;
        let add = -40;
        set_add(&StorageKey::default(), Some(set), add, SHARED)
    });

    assert_syscall_exception(
        &res,
        SyscallNo::kNonnegativeIntSetAdd,
        SyscallErrorCode::kSyscallErrorCodeInvalidSyscallInput,
    );
}

#[test]
// We can call `set_add(None, -3)`, but the transaction reverts.
fn negative_increment_reverts() {
    let result = run_tx(|| {
        set_add(&StorageKey::default(), None, -3, SHARED);
    });
    // The frame succeeds but the commit must fail.
    result.assert_commit_failed();
}

#[test]
// We can make serial `set_add` calls such that the total would be negative,
// but the transaction reverts.
fn negative_set_add_serially_reverts() {
    let result = run_tx(|| {
        let key = StorageKey::default();

        set_add(&key, Some(1), 0, SHARED);
        set_add(&key, None, -40, SHARED);
    });
    // The frame succeeds but the commit must fail.
    result.assert_commit_failed();
}

#[test]
// We can explicitly set a negative value, and the transaction succeeds.
fn negative_set_succeeds() {
    run_tx(|| {
        let set = -1;
        let add = 0;
        set_add(&StorageKey::default(), Some(set), add, SHARED)
    })
    .assert_success();
}

#[test]
// We can't complete a transaction with "set" and "add" values whose sum would
// overflow an `i128`.
fn set_overflowing_sum_reverts() {
    let result = run_tx(|| {
        let key = StorageKey::default();
        let set = i128::MAX;
        let add = 47;
        set_add(&key, Some(set), add, SHARED);
    });
    // The frame succeeds but the commit must fail.
    result.assert_commit_failed();
}
