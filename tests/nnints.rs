//! Test the semantics of the SDK's non_negative_int functions

#![allow(unused)]

use sdk::call::*;
use sdk::sys::SyscallNo;
use sdk::*;

contract_test!(test_nnints {
    let key = StorageKey::default();
    let key = &key;

    // Assuming no previous activity at this storage location, this is the effective
    // value stored there:
    let mut n = 0;

    // Incrementing a previously-unset location succeeds
    let inc = 3;
    non_negative_int_inc(key, inc);

    // Getting an incremented-only location succeeds as if the previous value had been zero
    n += inc;
    assert_eq!(non_negative_int_get(key), n);

    // Incrementing again succeeds
    let inc = 7;
    non_negative_int_inc(key, inc);

    // Getting yields the cumulative sum of increments, plus any set value from this transaction or
    // the value at the start of the transaction.
    n += inc;
    assert_eq!(non_negative_int_get(key), n);

    // Setting succeeds
    n = 27;
    non_negative_int_set(key, n);

    // Getting yields the just-set value
    assert_eq!(non_negative_int_get(key), n);

    // Incrementing from here succeeds
    let inc = 100;
    non_negative_int_inc(key, inc);

    // Getting yields the cumulative sum of increments, plus any set value from this transaction or
    // the value at the start of the transaction.
    n += inc;
    assert_eq!(non_negative_int_get(key), n);

    // Revert restores the value at the start of the transaction
    non_negative_int_revert(key);
    n = 0;
    assert_eq!(non_negative_int_get(key), n);

    // We can delete the location and then set again
    n = 0;
    delete_shared(key);
    non_negative_int_set(key, n);

    /* TODO: Unexpected transaction result: ExecResultTxNonDeterministicError
    // We can set the conditions for an overflow without failure ...
    n = i128::MAX;
    let inc = 5;
    non_negative_int_set(key, n).unwrap();
    non_negative_int_inc(key, inc).unwrap();

    // ... but retrieving the result fails
    assert!(matches!(non_negative_int_get(key), Err(Error::NonNegativeIntOverflow)));
    */
});

#[test]
fn get_unset_fails() {
    let mut env = TestEnvironment::no_cost();

    let res = contract_exec(env, || {
        // Getting a previously-unset location gives not-exist
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

#[test]
fn delete_actually_deletes() {
    let mut env = TestEnvironment::no_cost();

    let res = contract_exec(env, || {
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
