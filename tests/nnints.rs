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
    // operation is `set_add`
    let inc = 3;
    non_negative_int_inc(key, inc); n += inc;
    assert_eq!(non_negative_int_get(key), n);

    // Incrementing (again, via `set_add`) succeeds even if we put a value of
    // another storage type there:
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

    // Revert restores the value at the start of the transaction
    non_negative_int_restore(key); n = 0;
    assert_eq!(non_negative_int_get(key), n);

    // We can delete the location and then set again
    delete_shared(key);
    n = 0;
    non_negative_int_set(key, n);
});

#[test]
// We can't use "set" and then later make an increment that results
// in a negative value; this leads (in the second call below) to
// `set_add(Some(2), -5`, which causes a syscall error.
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
            ) => println!("OK: {}", exn),
        _ => panic!("Unexpected result: {:?}", res),
    }
}

#[test]
// Restore on a previously-unset location succeeds XXX(NNInt): Is this okay, and what is committed?
fn restore_unset_succeeds() {
    let res = contract_exec(TestEnvironment::no_cost(), || {
        let key = StorageKey::default();
        let key = &key;

        non_negative_int_restore(key);
        assert_eq!(non_negative_int_get(key), 0);
    });

    if !res.is_ok() {
        panic!("Unexpected failure: {:?}", res);
    }
}

#[test]
// Restore on a wrong-type location succeeds XXX(NNInt): Is this okay, and what is committed?
fn restore_wrong_type_succeeds() {
    let res = contract_exec(TestEnvironment::no_cost(), || {
        let key = StorageKey::default();
        let key = &key;

        bytestring_set(key, b"");
        non_negative_int_restore(key);
        assert_eq!(non_negative_int_get(key), 0);
    });

    if !res.is_ok() {
        panic!("Unexpected failure: {:?}", res);
    }
}

#[test]
// Getting a previously-unset location gives not-exist
fn get_unset_fails() {
    let res = contract_exec(TestEnvironment::no_cost(), || {
        let _ = non_negative_int_get(&StorageKey::default());
    });

    match res {
        Err(CallError::Abort(ref exn))
            if exn.is_syscall_exn(
                SyscallNo::kNonnegativeIntGet,
                SyscallErrorCode::kSyscallErrorCodeStorageKeyNotExist,
            ) => println!("OK: {}", exn),
        _ => panic!("Unexpected result: {:?}", res),
    }
}

#[test]
// Getting a wrong-type location gives invalid-input
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
            ) => println!("OK: {}", exn),
        _ => panic!("Unexpected result: {:?}", res),
    }
}

#[test]
// Try-get on a wrong-type location gives invalid-input
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
            ) => println!("OK: {}", exn),
        _ => panic!("Unexpected result: {:?}", res),
    }
}

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
            ) => println!("OK: {}", exn),
        _ => panic!("Unexpected result: {:?}", res),
    }
}
