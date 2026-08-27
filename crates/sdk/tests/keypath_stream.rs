//! Streamed storage-key derivation: `Keypath::generate_key` goes through the
//! VM's incremental-hash streams (with a cached, `SHA256Copy`-forked parent
//! midstate) and must produce byte-identical keys to the one-shot `sha256` of
//! the preimage, in every cache and fallback state.

use sdk::storage::Keypath;
use sdk::{Hasher, N_HASHERS, StorageKey, contract_test, hash_to_array};

// The golden vector from the keypath unit tests: the preimage of
// `child(&1u32).child(&0u32)` is [01 01 01 00].
const GOLDEN: StorageKey =
    StorageKey::new(hash_to_array!(b"\x01\x01\x01\x00"));

// A parented derivation (split inside the preimage) hits the fork path and
// still produces the exact preimage hash.
contract_test!(test_streamed_key_matches_golden {
    let kp = Keypath::default().child(&1u32).child(&0u32);
    assert_eq!(kp.generate_key(), GOLDEN);
});

// A root-level child (no parent prefix) takes the uncached full-feed path.
contract_test!(test_rootless_key_matches_golden {
    // Preimage of child(&1u32) alone: [01 01].
    const EXPECTED: StorageKey = StorageKey::new(hash_to_array!(b"\x01\x01"));
    assert_eq!(Keypath::default().child(&1u32).generate_key(), EXPECTED);
});

// Two siblings under one parent: the second derivation forks the cached
// parent midstate. Then a sibling under a DIFFERENT parent replaces the
// cache. Every key must still equal the direct preimage hash.
contract_test!(test_sibling_and_cache_replacement_keys {
    let parent_a = Keypath::default().child(&1u32);
    let parent_b = Keypath::default().child(&2u32);

    // Preimages: [01 01 | 01 00], [01 01 | 01 01], [01 02 | 01 00].
    const A0: StorageKey = StorageKey::new(hash_to_array!(b"\x01\x01\x01\x00"));
    const A1: StorageKey = StorageKey::new(hash_to_array!(b"\x01\x01\x01\x01"));
    const B0: StorageKey = StorageKey::new(hash_to_array!(b"\x01\x02\x01\x00"));

    assert_eq!(parent_a.child(&0u32).generate_key(), A0); // cache miss: builds A
    assert_eq!(parent_a.child(&1u32).generate_key(), A1); // cache hit: forks A
    assert_eq!(parent_b.child(&0u32).generate_key(), B0); // replaces A with B
    assert_eq!(parent_a.child(&0u32).generate_key(), A0); // replaces B with A again
});

// With every hasher slot held by the contract, derivation falls back to the
// one-shot sha256 syscall and the key does not change.
contract_test!(test_key_is_stable_under_slot_exhaustion {
    let kp = Keypath::default().child(&1u32).child(&0u32);
    assert_eq!(kp.generate_key(), GOLDEN);

    // Drain every remaining slot (the derivation's parent-midstate cache
    // legitimately pins one, so this yields N_HASHERS - 1).
    let mut held = Vec::new();
    while let Some(h) = Hasher::try_new() {
        held.push(h);
    }
    assert!(!held.is_empty(), "some hasher slots must be allocatable");
    assert!(Hasher::try_new().is_none(), "slots must be exhausted");

    assert_eq!(kp.generate_key(), GOLDEN);
    drop(held);

    assert_eq!(kp.generate_key(), GOLDEN);
});

// The derivation must not permanently consume hasher slots: after arbitrarily
// many generate_key calls, the contract can still allocate all but the one
// slot pinned by the parent-midstate cache.
contract_test!(test_derivation_pins_at_most_one_slot {
    for i in 0u32..20 {
        let _ = Keypath::default().child(&i).child(&i).generate_key();
    }

    let mut held = Vec::new();
    while let Some(h) = Hasher::try_new() {
        held.push(h);
    }
    assert!(
        held.len() >= N_HASHERS - 1,
        "expected at least {} free hasher slots, got {}",
        N_HASHERS - 1,
        held.len()
    );
});
