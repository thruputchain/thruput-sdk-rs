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

// A frame failure with every hasher slot live must unwind cleanly: the
// unwind drops each live `Hasher`, reclaiming its slot without panicking.
// A drop-time panic here would be a double panic — the whole test
// process would abort instead of reporting the failure. The companion unit
// test in `src/hasher.rs` pins `Hasher::drop` against a desynced slot
// (the state that used to panic); this test covers the guest-visible
// unwind-with-live-hashers path end to end.
#[test]
fn test_frame_failure_with_live_hashers_unwinds_cleanly() {
    let env = TestEnvironment::no_cost();
    let addr = ContractAddress::named("hasher_unwind");

    // Exhaust the slots so the failure is hasher-triggered: with all
    // N_HASHERS live, the next allocation throws the stream-cap syscall
    // exception, and the resulting unwind drops every live hasher.
    let result = env.transact(|tx| {
        tx.call(&addr).run(|| {
            let mut hashers = vec![];
            for _ in 0..N_HASHERS {
                let mut h = Hasher::new();
                h.update(b"foo");
                hashers.push(h);
            }
            assert!(Hasher::try_new().is_none(), "all slots must be in use");
            // Throws: no more hashers are available.
            let _overflow = Hasher::new();
        })
    });
    result.assert_abort();

    // The unwind reclaimed every slot: a fresh frame can allocate and
    // use all of them again.
    env.transact(|tx| {
        tx.call(&addr).run(|| {
            for _ in 0..N_HASHERS {
                let mut h = Hasher::new();
                h.update(b"foo");
                h.update(b"bar");
                assert_eq!(h.finish(), EXPECTED);
            }
        })
    })
    .assert_success();
}

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

// Forking copies the absorbed state exactly, and the two hashers diverge
// independently afterwards: prefix "foo" is absorbed once, then one fork
// finishes as "foobar" and the original as "foobaz".
const EXPECTED_BAZ: SHA256Hash = SHA256Hash::new(hash_to_array!(b"foobaz"));

contract_test!(test_hasher_fork_diverges {
    let mut h = Hasher::new();
    h.update(b"foo");

    let mut forked = h.fork();
    forked.update(b"bar");
    h.update(b"baz");

    assert_eq!(forked.finish(), EXPECTED);
    assert_eq!(h.finish(), EXPECTED_BAZ);
});

// Forking a brand-new (never-updated) hasher yields an equally fresh hasher;
// both still produce correct digests, including the empty input.
contract_test!(test_hasher_fork_of_fresh_hasher {
    let h = Hasher::new();
    let mut forked = h.fork();
    forked.update(b"foobar");
    assert_eq!(forked.finish(), EXPECTED);
    assert_eq!(h.finish(), EXPECTED_EMPTY);
});

// A fork consumes a slot: with all N_HASHERS live, try_fork must return None
// rather than panic, and forking works again once a slot frees up.
contract_test!(test_hasher_fork_respects_slot_budget {
    let mut hashers = vec![];
    for _ in 0..N_HASHERS {
        let mut h = Hasher::new();
        h.update(b"foo");
        hashers.push(h);
    }

    assert!(hashers[0].try_fork().is_none());

    // Free one slot, then the fork succeeds and carries the copied state.
    hashers.pop();
    let mut forked = hashers[0].fork();
    forked.update(b"bar");
    assert_eq!(forked.finish(), EXPECTED);
});
