//! Test the semantics of hashers

use sdk::*;

// This uses the sha2 module during macro expansion
const EXPECTED: SHA256Hash = SHA256Hash::new(hash_to_array!(b"foobar"));

contract_test!(test_hasher_simple {
    let mut h = Hasher::new();

    h.update(b"foo");
    h.update(b"bar");

    assert_eq!(h.finish(), EXPECTED);
});

contract_test!(test_hasher_exhaustion {
    // Allocate all the available hashers
    {
        let mut hashers = vec![];

        for _ in 0..N_HASHERS {
            let mut h = Hasher::new();
            h.update(b"foobar");
            hashers.push(h);
        }

        // Check and consume the hashers
        hashers.into_iter().for_each(|h| {
            assert_eq!(h.finish(), EXPECTED);
        });
    }

    // Check that we can allocate a new hasher after
    // the others have been dropped
    let _ = Hasher::new();
});
