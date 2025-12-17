//! Test the semantics of hashers

use sdk::*;

contract_test!(test_hasher {
    let mut hasher = Hasher::new();

    hasher.update(b"foo");
    hasher.update(b"bar");

    const EXPECTED: SHA256Hash = SHA256Hash::new(hash_to_array!(b"foobar"));

    assert_eq!(hasher.finish(), EXPECTED);
});

contract_test!(test_hasher_exhaustion {
    let mut hashers = vec![];

    for _ in 0..N_HASHERS {
        let mut h = Hasher::new();
        h.update(b"foobar");
        hashers.push(h);
    }

    // Check and consume the hashers
    hashers.into_iter().for_each(|mut h| {
        const EXPECTED: SHA256Hash = SHA256Hash::new(hash_to_array!(b"foobar"));

        assert_eq!(h.finish(), EXPECTED);
    });


    // Check that we can allocate a new hasher after
    // the others have been dropped
    let _ = Hasher::new();
});
