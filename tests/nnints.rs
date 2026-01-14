//! Test the semantics of the SDK's non_negative_int functions

#![allow(unused)]

use sdk::sysresult::Error;
use sdk::*;

#[cfg(any())] // XXX: ExecResultTxNonDeterministicError
contract_test!(test_nnints {
    let key = StorageKey::default();
    let key = &key;

    // Assuming no previous activity at this storage location, this is the effective
    // value stored there:
    let mut n = 0;

    // Getting a previously-unset location gives not-exist
    assert!(matches!(non_negative_int_get(key), Err(e) if e.is_storage_key_not_exist()));

    // Incrementing a previously-unset location succeeds
    let inc = 3;
    non_negative_int_inc(key, inc).unwrap();

    // Getting an incremented-only location succeeds as if the previous value had been zero
    n += inc;
    assert_eq!(non_negative_int_get(key), Ok(n));

    // Incrementing again succeeds
    let inc = 7;
    non_negative_int_inc(key, inc).unwrap();

    // Getting yields the cumulative sum of increments, plus any set value from this transaction or
    // the value at the start of the transaction.
    n += inc;
    assert_eq!(non_negative_int_get(key), Ok(n));

    // Setting succeeds
    n = 27;
    non_negative_int_set(key, n).unwrap();

    // Getting yields the just-set value
    assert_eq!(non_negative_int_get(key), Ok(n));

    // Incrementing from here succeeds
    let inc = 100;
    non_negative_int_inc(key, inc).unwrap();

    // Getting yields the cumulative sum of increments, plus any set value from this transaction or
    // the value at the start of the transaction.
    n += inc;
    assert_eq!(non_negative_int_get(key), Ok(n));

    // Revert restores the value at the start of the transaction
    non_negative_int_revert(key).unwrap();
    n = 0;
    assert_eq!(non_negative_int_get(key), Ok(n));

    // We can delete the location and then set again
    n = 0;
    delete_shared(key).unwrap();
    assert!(matches!(non_negative_int_get(key), Err(e) if e.is_storage_key_not_exist()));
    non_negative_int_set(key, n).unwrap();

    // We can set the conditions for an overflow without failure ...
    n = i128::MAX;
    let inc = 5;
    non_negative_int_set(key, n).unwrap();
    non_negative_int_inc(key, inc).unwrap();

    // ... but retrieving the result fails
    assert!(matches!(non_negative_int_get(key), Err(Error::NonNegativeIntOverflow)));
});
