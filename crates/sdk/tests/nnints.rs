//! Test the semantics of the SDK's non_negative_int functions

#![allow(unused)]

mod common;
use common::*;

use sdk::call::RawCallError;
use sdk::exn::{Exn, LibraryExn};
use sdk::sys::{SyscallErrorCode, SyscallNo};
use sdk::*;

/// Run `f` as a single transaction at this file's test address.
fn run_tx(f: impl FnOnce() + std::panic::UnwindSafe) -> TransactResult {
    let env = TestEnvironment::no_cost();
    env.transact(|tx| tx.call(&ContractAddress::named("nnints")).run(f))
}

/// Run `f` and decode the root-frame outcome as a client-style result,
/// for matching typed exceptions.
#[allow(clippy::result_large_err)]
fn run_tx_result(
    f: impl FnOnce() + std::panic::UnwindSafe,
) -> Result<(), RawCallError> {
    into_root_call_result(&run_tx(f))
}

contract_test!(test_nnints_exn {
    use sdk::sys_exn::nnint::*;
    use sdk::sys_exn::delete::*;

    static KEY: StorageKey = static_key!("my nnint");
    let key = &KEY;

    // Assuming no previous activity at this storage location, try_get gives
    // None:
    assert_eq!(non_negative_int_try_get(key), None);

    let mut n = 0;

    // Incrementing a previously-unset location succeeds, as the underlying
    // operation is `set_add(None, add)`.
    let inc = 3;
    non_negative_int_inc(key, inc); n += inc;
    assert_eq!(non_negative_int_get(key), n);

    // Incrementing (again, via `set_add`) succeeds even if a value of another
    // storage type is there:
    bytestring_set(key, b""); n = 0;
    non_negative_int_inc(key, inc); n += inc;
    assert_eq!(non_negative_int_get(key), n);

    // Incrementing again succeeds
    let inc = 7;
    non_negative_int_inc(key, inc); n += inc;

    // Getting yields the cumulative sum of increments, plus any set value from this transaction or
    // the value at the start of the transaction.
    assert_eq!(non_negative_int_get(key), n);

    // Setting succeeds
    n = 27;
    non_negative_int_set(key, n);

    // Getting yields the just-set value
    assert_eq!(non_negative_int_get(key), n);

    // Incrementing from here succeeds
    let inc = 100;
    non_negative_int_inc(key, inc); n += inc;

    // Getting yields the cumulative sum of increments, plus any set value from this transaction or
    // the value at the start of the transaction.
    assert_eq!(non_negative_int_get(key), n);

    // We can explicitly set a negative value
    n = -5;
    non_negative_int_set(key, n);
    assert_eq!(non_negative_int_get(key), n);

    // We can increment even if it remains negative
    let inc = 1;
    non_negative_int_inc(key, inc); n += inc;

    let _ = n;

    // We can delete the location and then set again
    delete_shared(key);
    n = 0;
    non_negative_int_set(key, n);
});

// After calling `set` with a negative number, increments are allowed even if
// they result in a negative sum again.  And the transaction succeeds.
//
#[test]
fn set_negative_then_increment_succeeds() {
    use sdk::sys_exn::nnint::*;

    run_tx(|| {
        let key = StorageKey::default();
        let key = &key;

        non_negative_int_set(key, -3);
        non_negative_int_inc(key, 1);
    })
    .assert_success();
}

// We can't set a positive value and then make an increment that results
// in a negative value.  This aborts.
//
// [This leads (in the second call below) to `set_add(Some(2), -5)`, which
// causes a syscall error.]
//
#[test]
fn set_negative_sum_fails() {
    use sdk::sys_exn::nnint::*;

    let res = run_tx_result(|| {
        let key = StorageKey::default();
        let key = &key;

        non_negative_int_set(key, 2);
        non_negative_int_inc(key, -5);
    });

    assert_syscall_exception(
        &res,
        SyscallNo::kNonnegativeIntSetAdd,
        SyscallErrorCode::kSyscallErrorCodeInvalidSyscallInput,
    );
}

// If the last set was positive, we can't do an increment that results in a
// negative.  This aborts.
//
// [Same mechanics as above.]
//
#[test]
fn set_negative_then_positive_then_negative_sum_fails() {
    use sdk::sys_exn::nnint::*;

    let res = run_tx_result(|| {
        let key = StorageKey::default();
        let key = &key;

        non_negative_int_set(key, -3);
        non_negative_int_inc(key, 1); // still negative: OK
        non_negative_int_set(key, 5);
        non_negative_int_inc(key, -7); // Nope: last set was positive
    });

    assert_syscall_exception(
        &res,
        SyscallNo::kNonnegativeIntSetAdd,
        SyscallErrorCode::kSyscallErrorCodeInvalidSyscallInput,
    );
}

// Getting a previously-unset location aborts with not-exist.
//
#[test]
fn get_unset_fails() {
    use sdk::sys_exn::nnint::*;

    let res = run_tx_result(|| {
        let _ = non_negative_int_get(&StorageKey::default());
    });

    assert_syscall_exception(
        &res,
        SyscallNo::kNonnegativeIntGet,
        SyscallErrorCode::kSyscallErrorCodeStorageKeyNotExist,
    );
}

// Getting a wrong-type location aborts with invalid-input.
//
#[test]
fn get_wrong_type_fails() {
    use sdk::sys_exn::nnint::*;

    let res = run_tx_result(|| {
        let key = StorageKey::default();
        let key = &key;

        bytestring_set(key, b"");
        let _ = non_negative_int_get(key);
    });

    assert_syscall_exception(
        &res,
        SyscallNo::kNonnegativeIntGet,
        SyscallErrorCode::kSyscallErrorCodeInvalidSyscallInput,
    );
}

// Try-get on a wrong-type location aborts with invalid-input.
//
#[test]
fn try_get_wrong_type_fails() {
    use sdk::sys_exn::nnint::*;

    let res = run_tx_result(|| {
        let key = StorageKey::default();
        let key = &key;

        bytestring_set(key, b"");
        let _ = non_negative_int_try_get(key);
    });

    assert_syscall_exception(
        &res,
        SyscallNo::kNonnegativeIntGet,
        SyscallErrorCode::kSyscallErrorCodeInvalidSyscallInput,
    );
}

// Trying to get an nnint from a deleted key aborts with not-exist.
//
#[test]
fn delete_actually_deletes() {
    use sdk::sys_exn::delete::*;
    use sdk::sys_exn::nnint::*;

    let res = run_tx_result(|| {
        let key = StorageKey::default();
        let key = &key;

        non_negative_int_set(key, 27);
        delete_shared(key);
        let _ = non_negative_int_get(key);
    });

    assert_syscall_exception(
        &res,
        SyscallNo::kNonnegativeIntGet,
        SyscallErrorCode::kSyscallErrorCodeStorageKeyNotExist,
    );
}
