use sdk::*;

#[rustfmt::skip]
contract_test!(test_nnints {
    let key = StorageKey::default();
    let key = &key;

    let mut set = 0;
    let mut add = 0;

        // Get (without forcing) on a previously-unset location returns "add" == zero
        assert_eq!(non_negative_int_get_values(key, false), Ok((None, add)));

        // Get (forcing) on a previously-unset location fails
        assert!(matches!(non_negative_int_get_values(key, true), Err(e) if e.is_storage_key_not_exist()));

    // Add to a previously-unset location succeeds
    add = 100;
    assert_eq!(non_negative_int_add(key, add), Ok(()));

        // We get just the "add" value back when not forcing
        assert_eq!(non_negative_int_get_values(key, false), Ok((None, add)));

        // We get the "set" and "add" values back when forcing; "set" has defaulted to zero
        assert_eq!(non_negative_int_get_values(key, true), Ok((Some(set), add)));

    // Adding again to the location succeeds
    add = 3;
    assert_eq!(non_negative_int_add(key, add), Ok(()));

        // The "add" value is now what we provided in the last call, NOT a running sum
        assert_eq!(non_negative_int_get_values(key, true), Ok((Some(set), add)));

        // We still get just the "add" value back when not forcing, even though it
        // has been forced (but not set) before
        assert_eq!(non_negative_int_get_values(key, false), Ok((None, add)));

    // We finally provide a "set" value, clearing "add"
    set = 7;
    add = 0;
    assert_eq!(non_negative_int_set(key, set), Ok(()));

        // Even if we don't force, we'll see the cached "set" value
        assert_eq!(non_negative_int_get_values(key, false), Ok((Some(set), add)));

        // Same happens if we do force
        assert_eq!(non_negative_int_get_values(key, true), Ok((Some(set), add)));

    // Here we just specify "add" again ...
    add = 17;
    assert_eq!(non_negative_int_add(key, add), Ok(()));

        // The unspecified "set" value HAS BEEN CLEARED TO ZERO
        set = 0;
        assert_eq!(non_negative_int_get_values(key, true), Ok((Some(set), add)));

    // We can provide "set" and "add" at the same time, for whatever that's
    // worth
    set = 8;
    add = 2;
    assert_eq!(non_negative_int_set_add(key, Some(set), add), Ok(()));

        // They've both been updated
        assert_eq!(non_negative_int_get_values(key, true), Ok((Some(set), add)));

    // TODO: test behavior with `delete` and changes of value-type (using
    // `bytestring_set`, etc.)
});
