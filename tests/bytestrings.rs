//! Test the semantics of the database (i.e., key-value store)

// use sdk::sysresult::Error;
use sdk::*;

contract_test!(test_bytestring {
    let key = StorageKey::default();
    let key = &key;

    // Getting a previously-unset location gives a not-exist error (or None)
    assert!(matches!(bytestring_get(key), Err(e) if e.is_storage_key_not_exist()));
    assert!(matches!(bytestring_get_len(key), Err(e) if e.is_storage_key_not_exist()));
    assert!(matches!(bytestring_try_get(key), Ok(None)));

    // Deleting any location is okay
    assert!(matches!(delete_shared(key), Ok(())));

    // Getting a deleted location gives not-exist (or None)
    assert!(matches!(bytestring_get(key), Err(e) if e.is_storage_key_not_exist()));
    assert!(matches!(bytestring_get_len(key), Err(e) if e.is_storage_key_not_exist()));
    assert!(matches!(bytestring_try_get(key), Ok(None)));

    // Getting a location holding an nnint gives "invalid input"
    assert!(matches!(non_negative_int_set(key, 47), Ok(())));
    assert!(matches!(bytestring_get(key), Err(e) if e.is_invalid_input()));
    assert!(matches!(bytestring_get_len(key), Err(e) if e.is_invalid_input()));

    // Setting the bytestring at a location holding an another type of object is okay
    assert!(matches!(bytestring_set(key, b"hello"), Ok(())));

    // Setting the bytestring at a deleted location is also okay
    assert!(matches!(delete_shared(key), Ok(())));
    assert!(matches!(bytestring_set(key, b"hello"), Ok(())));
});
