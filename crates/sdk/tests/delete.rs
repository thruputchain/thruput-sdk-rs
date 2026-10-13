//! Test the semantics of `delete`

use thruput_sdk::sys_result::delete::delete_shared;
use thruput_sdk::*;

contract_test!(test_delete_unstored {
    let key = StorageKey::default();
    let key = &key;

    // Deleting a previously-unstored location is okay
    assert!(matches!(delete_shared(key), Ok(())));
});
