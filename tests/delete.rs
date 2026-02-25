//! Test the semantics of `delete`

use sdk::*;

contract_test!(test_delete_unstored {
    let key = StorageKey::default();
    let key = &key;

    // Deleting a previously-unstored location is okay
    assert!(matches!(try_delete_shared(key), Ok(())));
});
