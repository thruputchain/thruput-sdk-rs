//! Test the semantics of the raw nnint syscalls

use sdk::internal::nnint::*;
use sdk::*;

#[rustfmt::skip]
contract_test!(test_sys_nnints {
    let key = StorageKey::default();
    let key = &key;

    // Assuming no previous activity at this storage location, these are the values
    // stored there:
    let mut set = 0;
    let mut add = 0;

        // Get (without forcing) on a previously-unset location returns "add" == zero
        assert_eq!(get_values(key), Ok((None, add)));

        // Get (forcing) on a previously-unset location fails
        assert!(matches!(get_values_force(key), Err(e) if e.is_storage_key_not_exist()));

    // `set_add` (specifying only "add") to a previously-unset location succeeds
    add = 100;
    set_add(key, None, add).unwrap();

        // We get just the "add" value back when not forcing
        assert_eq!(get_values(key), Ok((None, add)));

        // We get the "set" and "add" values back when forcing; "set" was restored by the `set_add`
        // above to its state at the start of the transaction
        assert_eq!(get_values_force(key), Ok((set, add)));

    // `set_add` (specifying only "add") again succeeds
    add = 3;
    set_add(key, None, add).unwrap();

        // The "add" value is now what we provided in the last call, NOT a running sum
        assert_eq!(get_values_force(key), Ok((set, add)));

        // We still get just the "add" value back when not forcing, even though it
        // has been forced (but not set) before
        assert_eq!(get_values(key), Ok((None, add)));

    // We now provide a non-zero "set" value, and clear "add"
    set = 7;
    add = 0;
    set_add(key, Some(set), add).unwrap();

        // Even if we don't force, we'll see the transaction's latest "set" value
        assert_eq!(get_values(key), Ok((Some(set), add)));

        // Same happens if we do force
        assert_eq!(get_values_force(key), Ok((set, add)));

    // Here we just specify "add" again ...
    add = 17;
    set_add(key, None, add).unwrap();

        // The unspecified "set" value has been restored to its value at the start of the
        // transaction
        set = 0;
        assert_eq!(get_values_force(key), Ok((set, add)));

    // We can also provide "set" and "add" at the same time, for whatever that's worth
    set = 8;
    add = 2;
    set_add(key, Some(set), add).unwrap();

        // They've both been updated
        assert_eq!(get_values_force(key), Ok((set, add)));

    // We can explicitly set a negative value
    add = -40;
    set_add(key, None, add).unwrap();

        set = 0;
        assert_eq!(get_values_force(key), Ok((set, add)));

    // We can set "set" and "add" to values whose sum would overflow an `i128`
    set = i128::MAX;
    add = 47;
    set_add(key, Some(set), add).unwrap();

        assert_eq!(get_values_force(key), Ok((set, add)));

    // TODO: test overflows
    // TODO: test with `delete`
    // TODO: test with changes of value-type (using `bytestring_set`, etc.)
});
