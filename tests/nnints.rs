//! Test the semantics of the SDK's non_negative_int functions

#![allow(unused)]

use sdk::call::*;
use sdk::sys::{SyscallErrorCode, SyscallNo};
use sdk::*;

contract_test!(test_nnints {
    let key = StorageKey::default();
    let key = &key;

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

    // Reset restores the value at the start of the transaction, if any.
    non_negative_int_reset(key); n = 0;
    assert_eq!(non_negative_int_get(key), n);

    // Reset even destroys values of other types
    bytestring_set(key, b"");
    non_negative_int_reset(key); n = 0;
    assert_eq!(non_negative_int_get(key), n);

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
    let res = contract_exec(TestEnvironment::no_cost(), || {
        let key = StorageKey::default();
        let key = &key;

        non_negative_int_set(key, -3);
        non_negative_int_inc(key, 1);
    });

    if !res.is_ok() {
        panic!("Unexpected failure: {:?}", res);
    }
}

// We can't set a positive value and then make an increment that results
// in a negative value.  This aborts.
//
// [This leads (in the second call below) to `set_add(Some(2), -5)`, which
// causes a syscall error.]
//
#[test]
fn set_negative_sum_fails() {
    let res = contract_exec(TestEnvironment::no_cost(), || {
        let key = StorageKey::default();
        let key = &key;

        non_negative_int_set(key, 2);
        non_negative_int_inc(key, -5);
    });

    match res {
        Err(CallError::Abort(ref exn))
            if exn.is_syscall_exn(
                SyscallNo::kNonnegativeIntSetAdd,
                SyscallErrorCode::kSyscallErrorCodeInvalidSyscallInput,
            ) =>
        {
            println!("OK: {}", exn)
        }
        _ => panic!("Unexpected result: {:?}", res),
    }
}

// If the last set was positive, we can't do an increment that results in a
// negative.  This aborts.
//
// [Same mechanics as above.]
//
#[test]
fn set_negative_then_positive_then_negative_sum_fails() {
    let res = contract_exec(TestEnvironment::no_cost(), || {
        let key = StorageKey::default();
        let key = &key;

        non_negative_int_set(key, -3);
        non_negative_int_inc(key, 1); // still negative: OK
        non_negative_int_set(key, 5);
        non_negative_int_inc(key, -7); // Nope: last set was positive
    });

    match res {
        Err(CallError::Abort(ref exn))
            if exn.is_syscall_exn(
                SyscallNo::kNonnegativeIntSetAdd,
                SyscallErrorCode::kSyscallErrorCodeInvalidSyscallInput,
            ) =>
        {
            println!("OK: {}", exn)
        }
        _ => panic!("Unexpected result: {:?}", res),
    }
}

// Getting a previously-unset location aborts with not-exist.
//
#[test]
fn get_unset_fails() {
    let res = contract_exec(TestEnvironment::no_cost(), || {
        let _ = non_negative_int_get(&StorageKey::default());
    });

    match res {
        Err(CallError::Abort(ref exn))
            if exn.is_syscall_exn(
                SyscallNo::kNonnegativeIntGet,
                SyscallErrorCode::kSyscallErrorCodeStorageKeyNotExist,
            ) =>
        {
            println!("OK: {}", exn)
        }
        _ => panic!("Unexpected result: {:?}", res),
    }
}

// Getting a wrong-type location aborts with invalid-input.
//
#[test]
fn get_wrong_type_fails() {
    let res = contract_exec(TestEnvironment::no_cost(), || {
        let key = StorageKey::default();
        let key = &key;

        bytestring_set(key, b"");
        let _ = non_negative_int_get(key);
    });

    match res {
        Err(CallError::Abort(ref exn))
            if exn.is_syscall_exn(
                SyscallNo::kNonnegativeIntGet,
                SyscallErrorCode::kSyscallErrorCodeInvalidSyscallInput,
            ) =>
        {
            println!("OK: {}", exn)
        }
        _ => panic!("Unexpected result: {:?}", res),
    }
}

// Try-get on a wrong-type location aborts with invalid-input.
//
#[test]
fn try_get_wrong_type_fails() {
    let res = contract_exec(TestEnvironment::no_cost(), || {
        let key = StorageKey::default();
        let key = &key;

        bytestring_set(key, b"");
        let _ = non_negative_int_try_get(key);
    });

    match res {
        Err(CallError::Abort(ref exn))
            if exn.is_syscall_exn(
                SyscallNo::kNonnegativeIntGet,
                SyscallErrorCode::kSyscallErrorCodeInvalidSyscallInput,
            ) =>
        {
            println!("OK: {}", exn)
        }
        _ => panic!("Unexpected result: {:?}", res),
    }
}

// Trying to get an nnint from a deleted key aborts with not-exist.
//
#[test]
fn delete_actually_deletes() {
    let res = contract_exec(TestEnvironment::no_cost(), || {
        let key = StorageKey::default();
        let key = &key;

        non_negative_int_set(key, 27);
        delete_shared(key);
        let _ = non_negative_int_get(key);
    });

    match res {
        Err(CallError::Abort(ref exn))
            if exn.is_syscall_exn(
                SyscallNo::kNonnegativeIntGet,
                SyscallErrorCode::kSyscallErrorCodeStorageKeyNotExist,
            ) =>
        {
            println!("OK: {}", exn)
        }
        _ => panic!("Unexpected result: {:?}", res),
    }
}

// Reset (`set_add(None, 0)`) on a previously-unset location succeeds.
//
// XXX: Does this commit an nnint with value zero?
//
#[test]
fn reset_unset_succeeds() {
    let res = contract_exec(TestEnvironment::no_cost(), || {
        let key = StorageKey::default();
        let key = &key;

        non_negative_int_reset(key);
        assert_eq!(non_negative_int_get(key), 0);
    });

    if !res.is_ok() {
        panic!("Unexpected failure: {:?}", res);
    }
}

// Restore (`set_add(None, 0)`) on wrong-type location succeeds.
//
// XXX: Does this commit an nnint with value zero?
//
#[test]
fn reset_wrong_type_succeeds() {
    let res = contract_exec(TestEnvironment::no_cost(), || {
        let key = StorageKey::default();
        let key = &key;

        bytestring_set(key, b"");
        non_negative_int_reset(key);
        assert_eq!(non_negative_int_get(key), 0);
    });

    if !res.is_ok() {
        panic!("Unexpected failure: {:?}", res);
    }
}
