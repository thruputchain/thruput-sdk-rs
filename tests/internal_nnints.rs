//! Test the semantics of the raw nnint syscalls

#![allow(unused)]

use sdk::call::*;
use sdk::sys::{SyscallNo, SyscallErrorCode};
use sdk::internal::nnint::*;
use sdk::*;

#[rustfmt::skip]
contract_test!(test_internal_nnints {
    let key = StorageKey::default();
    let key = &key;

    // Assuming no previous activity at this storage location, these are the values
    // stored there:
    let mut set = 0;
    let mut add = 0;

        // Get (without forcing) on a location not yet set in this transaction returns "add" == zero
        assert_eq!(get_values(key), (None, add));

    // `set_add` (specifying only "add") to a previously-unset location succeeds
    add = 100;
    set_add(key, None, add);

        // We get just the "add" value back when not forcing
        assert_eq!(get_values(key), (None, add));

        // We get the "set" and "add" values back when forcing; "set" was restored by the `set_add`
        // above to its state at the start of the transaction
        assert_eq!(get_values_force(key), (set, add));

    // `set_add` (specifying only "add") again succeeds
    add = 3;
    set_add(key, None, add);

        // The "add" value is now what we provided in the last call, NOT a running sum
        assert_eq!(get_values_force(key), (set, add));

        // We still get just the "add" value back when not forcing, even though it
        // has been forced (but not set) before
        assert_eq!(get_values(key), (None, add));

    // We now provide a non-zero "set" value, and clear "add"
    set = 7;
    add = 0;
    set_add(key, Some(set), add);

        // Even if we don't force, we'll see the transaction's latest "set" value
        assert_eq!(get_values(key), (Some(set), add));

        // Same happens if we do force
        assert_eq!(get_values_force(key), (set, add));

    // Here we just specify "add" again ...
    add = 17;
    set_add(key, None, add);

        // The unspecified "set" value has been restored to its value at the start of the
        // transaction
        set = 0;
        assert_eq!(get_values_force(key), (set, add));

    // We can also provide "set" and "add" at the same time, for whatever that's worth
    set = 8;
    add = 2;
    set_add(key, Some(set), add);

        // They've both been updated
        assert_eq!(get_values_force(key), (set, add));

    // TODO: Unexpected transaction result: ExecResultTxNonDeterministicError
    /*
    // We can explicitly give a "set" value that is negative
    set = -4;
    add = 0;
    set_add(key, Some(set), add);

        assert_eq!(get_values_force(key), (set, add));
    */

    // TODO: Unexpected transaction result: ExecResultTxNonDeterministicError
    /*
    // We can set "set" and "add" to values whose sum would overflow an `i128`
    set = i128::MAX;
    add = 47;
    set_add(key, Some(set), add);

        assert_eq!(get_values_force(key), (set, add));
    */

    // TODO: test overflows
    // TODO: test with `delete`
    // TODO: test with changes of value-type (using `bytestring_set`, etc.)
});

#[test]
fn force_get_on_empty_fails() {
    let mut env = TestEnvironment::no_cost();

    let res = contract_exec(env, || {
        // Get (forcing) on a never-before-set location fails
        get_values_force(&StorageKey::default());
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
fn negative_sum_fails() {
    let mut env = TestEnvironment::no_cost();

    let res = contract_exec(env, || {
        // We can't give "set" and "add" values whose sum would be negative
        let set = 1;
        let add = -40;
        set_add(&StorageKey::default(), Some(set), add)
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
