//! Test the semantics of hashers

use sdk::*;

contract_test!(test_bytestring {
    let mut hasher = Hasher::new();

    hasher.update(b"foo");
    hasher.update(b"bar");

    const EXPECTED: SHA256Hash = SHA256Hash::new(hash_to_array!(b"foobar"));

    assert_eq!(hasher.finish(), EXPECTED);
});
