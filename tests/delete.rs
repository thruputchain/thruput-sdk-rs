//! Test the semantics of `delete`

use sdk::sys_result::delete::delete_shared;
use sdk::*;

contract_test!(test_delete_unstored {
    let key = StorageKey::default();
    let key = &key;

    // Deleting a previously-unstored location is okay
    assert!(matches!(delete_shared(key), Ok(())));
});
