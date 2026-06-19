//! Test the semantics of hashers

use sdk::*;

// This uses the sha2 module during macro expansion
const EXPECTED: SHA256Hash = SHA256Hash::new(hash_to_array!(b"foobar"));

// The SHA-256 of the empty input.
const EXPECTED_EMPTY: SHA256Hash = SHA256Hash::new(hash_to_array!(b""));

contract_test!(test_hasher {
    // Allocate and use a hasher but don't finalize it.
    // (The VM slot should be reclaimed due to the hasher's
    // drop implementation.)
    {
        let mut h = Hasher::new();
        h.update(b"foo");
    }

    // Allocate all the available hashers
    {
        let mut hashers = vec![];

        for _ in 0..N_HASHERS {
            let mut h = Hasher::new();
            h.update(b"foo");
            h.update(b"bar");
            hashers.push(h);
        }

        // Check and consume the hashers
        hashers.into_iter().for_each(|h| {
            assert_eq!(h.finish(), EXPECTED);
        });
    }

    // Check that we can allocate a new hasher after
    // the others have been dropped
    let mut h = Hasher::new();
    h.update(b"foo");
    h.update(b"bar");
    assert_eq!(h.finish(), EXPECTED);
});

// Hashing the empty input on a brand-new (never-updated) hasher must return
// the SHA-256 of the empty string. Without the fix, `finish()` would call
// SHA256Finalize on a slot that the VM never allocated a buffer for, and the
// syscall would fail.
contract_test!(test_hasher_empty_input {
    assert_eq!(Hasher::new().finish(), EXPECTED_EMPTY);

    // Reachable the same way via Default.
    assert_eq!(Hasher::default().finish(), EXPECTED_EMPTY);
});

// The stale-slot scenario: a hasher is updated and then dropped without being
// finalized, so the VM's buffer for that slot still holds its input. A fresh
// empty hasher that reuses the same slot must still return the empty-input
// hash, not the dropped hasher's digest.
contract_test!(test_hasher_empty_input_stale_slot {
    {
        let mut h = Hasher::new();
        h.update(b"foobar");
        // Dropped without `finish()`, leaving a stale VM buffer behind.
    }

    // Reuses the just-freed slot; with the bug this would finalize the stale
    // "foobar" buffer and return EXPECTED instead of EXPECTED_EMPTY.
    assert_eq!(Hasher::new().finish(), EXPECTED_EMPTY);
});
