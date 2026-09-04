//! Test the semantics of hashers

use sdk::*;

// This uses the sha2 module during macro expansion
const EXPECTED: SHA256Hash = SHA256Hash::new(hash_to_array!(b"foobar"));

// The SHA-256 of the empty input.
const EXPECTED_EMPTY: SHA256Hash = SHA256Hash::new(hash_to_array!(b""));

contract_test!(test_hasher {
    // Allocate and use a hasher but don't finalize it: its drop
    // implementation frees the VM-side stream, so the full budget is
    // available again afterwards.
    {
        let mut h = Hasher::new();
        h.update(b"foo");
    }

    // Allocate all the available hashers: the VM enforces the live-stream
    // budget, so fill until it refuses.
    {
        let mut hashers = vec![];

        while let Ok(mut h) = Hasher::try_new() {
            h.update(b"foo");
            h.update(b"bar");
            hashers.push(h);
        }
        assert!(!hashers.is_empty(), "some streams must be allocatable");

        // The stream budget is spent.
        assert!(Hasher::try_new().is_err());

        // Check and consume the hashers — finishing frees their streams.
        hashers.into_iter().for_each(|h| {
            assert_eq!(h.finish(), EXPECTED);
        });
    }

    // The finished hashers freed their streams: allocation works again.
    let mut h = Hasher::new();
    h.update(b"foo");
    h.update(b"bar");
    assert_eq!(h.finish(), EXPECTED);
});

// Hashing the empty input on a brand-new (never-updated) hasher must return
// the SHA-256 of the empty string: construction creates the VM-side stream
// eagerly, so finalize sees an empty stream rather than a missing one.
contract_test!(test_hasher_empty_input {
    assert_eq!(Hasher::new().finish(), EXPECTED_EMPTY);

    // Reachable the same way via Default.
    assert_eq!(Hasher::default().finish(), EXPECTED_EMPTY);
});

// A frame failure with every hasher live must unwind cleanly, and stream
// exhaustion must surface as the refusing syscall's typed abort rather
// than a fault. Streams are per-invocation, so a fresh frame starts with
// the full budget again regardless of what the failed frame leaked.
#[test]
fn test_frame_failure_with_live_hashers_unwinds_cleanly() {
    let env = TestEnvironment::no_cost();
    let addr = ContractAddress::named("hasher_unwind");

    // The VM's live-stream budget is the chain's `max_sha256_buffers`
    // network config parameter — read it off the environment rather than
    // duplicating the value here.
    let cap = env.get_network_config().max_sha256_buffers as usize;
    assert!(cap > 0);

    // Exhaust the streams so the failure is hasher-triggered: with `cap`
    // hashers live, the next allocation throws the stream-cap syscall
    // exception, and the resulting unwind drops every live hasher.
    let result = env.transact(|tx| {
        tx.call(&addr).run(move || {
            let mut hashers = vec![];
            for _ in 0..cap {
                let mut h = Hasher::new();
                h.update(b"foo");
                hashers.push(h);
            }
            assert!(Hasher::try_new().is_err(), "all streams must be in use");
            // Throws: the VM refuses a stream past the cap.
            let _overflow = Hasher::new();
        })
    });
    result.assert_abort();

    // Streams are per-frame: a fresh frame can allocate and use the full
    // budget again.
    env.transact(|tx| {
        tx.call(&addr).run(move || {
            for _ in 0..cap {
                let mut h = Hasher::new();
                h.update(b"foo");
                h.update(b"bar");
                assert_eq!(h.finish(), EXPECTED);
            }
        })
    })
    .assert_success();
}

// A dropped hasher's state never bleeds into a new one: drop discards its
// stream and indices are never reused, so a fresh hasher starts from a
// fresh, empty stream.
contract_test!(test_hasher_empty_input_stale_slot {
    {
        let mut h = Hasher::new();
        h.update(b"foobar");
        // Dropped without `finish()`; drop discards the stream.
    }

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

// A fork consumes a stream: with every stream live, try_fork must return
// the refusing syscall's error rather than panic, and forking works
// again once a stream frees up.
contract_test!(test_hasher_fork_respects_slot_budget {
    let mut hashers = vec![];
    while let Ok(mut h) = Hasher::try_new() {
        h.update(b"foo");
        hashers.push(h);
    }
    assert!(!hashers.is_empty(), "some streams must be allocatable");

    assert!(hashers[0].try_fork().is_err());

    // Drop one hasher to free its stream, then the fork succeeds and
    // carries the copied state.
    hashers.pop();
    let mut forked = hashers[0].fork();
    forked.update(b"bar");
    assert_eq!(forked.finish(), EXPECTED);
});
