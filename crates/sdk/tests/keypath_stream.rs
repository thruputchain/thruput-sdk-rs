//! Streamed storage-key derivation: a `Keypath` IS a VM hash-stream
//! midstate — `child` forks the parent's stream and absorbs one frame,
//! `generate_key` forks once more and finalizes — and must produce exactly
//! the sha256 of the framed preimage.

use sdk::storage::Keypath;
use sdk::*;

// The golden vector from the keypath unit tests: the preimage of
// `child(&1u32).child(&0u32)` is [01 01 01 00].
const GOLDEN: StorageKey =
    StorageKey::new(hash_to_array!(b"\x01\x01\x01\x00"));

contract_test!(test_streamed_key_matches_golden {
    let kp = Keypath::default().child(&1u32).child(&0u32);
    assert_eq!(kp.generate_key(), GOLDEN);
});

contract_test!(test_rootless_key_matches_golden {
    // Preimage of child(&1u32) alone: [01 01].
    const EXPECTED: StorageKey = StorageKey::new(hash_to_array!(b"\x01\x01"));
    assert_eq!(Keypath::default().child(&1u32).generate_key(), EXPECTED);
});

// Siblings fork the shared parent midstate (the parent prefix is absorbed
// once, in the parent); every key must equal the direct preimage hash.
contract_test!(test_sibling_keys {
    let parent_a = Keypath::default().child(&1u32);
    let parent_b = Keypath::default().child(&2u32);

    // Preimages: [01 01 | 01 00], [01 01 | 01 01], [01 02 | 01 00].
    const A0: StorageKey = StorageKey::new(hash_to_array!(b"\x01\x01\x01\x00"));
    const A1: StorageKey = StorageKey::new(hash_to_array!(b"\x01\x01\x01\x01"));
    const B0: StorageKey = StorageKey::new(hash_to_array!(b"\x01\x02\x01\x00"));

    assert_eq!(parent_a.child(&0u32).generate_key(), A0);
    assert_eq!(parent_a.child(&1u32).generate_key(), A1);
    assert_eq!(parent_b.child(&0u32).generate_key(), B0);
    assert_eq!(parent_a.child(&0u32).generate_key(), A0);
});

// Keypaths, user hashers, and derivation transients share the VM's
// live-stream budget. Holding every stream makes the next keypath
// operation throw the refusing syscall's typed abort; streams are
// per-frame, so a fresh frame has the full budget again.
#[test]
fn test_exhaustion_throws_typed_abort() {
    let env = TestEnvironment::no_cost();
    let addr = ContractAddress::named("keypath_exhaustion");

    let result = env.transact(|tx| {
        tx.call(&addr).run(|| {
            let kp = Keypath::default().child(&1u32).child(&0u32);
            assert_eq!(kp.generate_key(), GOLDEN);

            let mut held = Vec::new();
            while let Ok(h) = Hasher::try_new() {
                held.push(h);
            }
            assert!(!held.is_empty(), "some streams must be allocatable");

            // No stream left for generate_key's transient fork: throws.
            let _ = kp.generate_key();
        })
    });
    result.assert_abort();

    env.transact(|tx| {
        tx.call(&addr).run(|| {
            let kp = Keypath::default().child(&1u32).child(&0u32);
            assert_eq!(kp.generate_key(), GOLDEN);
        })
    })
    .assert_success();
}

// Derivation must not permanently consume streams: transients are
// finalized and dropped keypaths free theirs, so after arbitrarily many
// derivations the full budget — the chain's `max_sha256_buffers` network
// config parameter, read off the environment — is allocatable again.
#[test]
fn test_derivation_pins_no_streams() {
    let env = TestEnvironment::no_cost();
    let cap = env.get_network_config().max_sha256_buffers as usize;
    assert!(cap > 0);

    env.transact(|tx| {
        tx.call(&ContractAddress::named("keypath_streams"))
            .run(move || {
                for i in 0u32..20 {
                    let _ =
                        Keypath::default().child(&i).child(&i).generate_key();
                }

                let mut held = Vec::new();
                while let Ok(h) = Hasher::try_new() {
                    held.push(h);
                }
                assert_eq!(held.len(), cap, "every stream must be free again");
            })
    })
    .assert_success();
}
