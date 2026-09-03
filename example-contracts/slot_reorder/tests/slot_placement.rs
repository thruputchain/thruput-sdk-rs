use sdk::storage::StorageHandle;
use std::cell::Cell;
use std::panic::AssertUnwindSafe;

use slot_reorder::*;

static NONCE: SHA256Hash = sdk::sha256_static!("SLOT_REORDER_NONCE");

/// Slots >= 63 take multi-byte CXS index encodings (2-, 3-, and 5-byte
/// forms). Lock the derive's field derivation against runtime `child()`
/// across every width, including the u32::MAX extreme the `#[slot(N)]`
/// parser allows.
#[derive(Storage)]
struct WideSlots {
    #[slot(63)]
    a: Value<u64>,
    #[slot(300)]
    b: Value<u64>,
    #[slot(70_000)]
    c: Value<u64>,
    #[slot(4_294_967_295)]
    d: Value<u64>,
}

fn setup() -> (TestEnvironmentHandle, SHA256Hash) {
    let env = TestEnvironment::no_cost();
    let contract = slot_reorder::register_mock_contract(&env);
    let contract_hash = contract.as_hash().clone();
    (env, contract_hash)
}

/// Verify that `#[slot(N)]` pins control where data is stored, not declaration order.
///
/// SlotReorder has:
///   - `first`  declared first, `#[slot(1)]`  → lives at child(&1u32)
///   - `second` declared second, `#[slot(0)]` → lives at child(&0u32)
///   - `third`  declared third, no pin         → lives at child(&2u32) (positional)
#[test]
fn slot_pins_control_storage_placement() {
    let (env, contract_hash) = setup();
    let addr = Cell::new(ContractAddress::default());

    let deploy = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(AssertUnwindSafe(|| {
                addr.set(sdk::deploy_contract(&NONCE, &contract_hash));
            }))
    });
    deploy.assert_success();
    env.advance_block();

    // Write distinct values through each field.
    let write = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(AssertUnwindSafe(|| {
                let c = SlotReorderClient::new(&addr.get());
                c.set_first(11);
                c.set_second(22);
                c.set_third(33);
            }))
    });
    write.assert_success();
    env.advance_block();

    // --- Verify data reads back via the contract API ---
    let read = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(AssertUnwindSafe(|| {
                let c = SlotReorderClient::new(&addr.get());
                assert_eq!(c.get_first(), 11);
                assert_eq!(c.get_second(), 22);
                assert_eq!(c.get_third(), 33);
            }))
    });
    read.assert_success();
    env.advance_block();

    // --- Verify via direct keypath construction that `first` lives at slot 1,
    //     not slot 0 (its declaration position). ---
    //
    // Run this verification in the deployed contract's own call context
    // (tx.call(&addr.get())) so that storage lookups are scoped to the
    // same contract address that stored the data.
    //
    // The storage root for SlotReorder is:
    //   Keypath::default().child("SlotReorder")
    // Each field is a child of that root keyed by its u32 slot index.
    //
    // `first` is declared first (position 0) but pinned to slot 1:
    //   data written as 11 must be found at child(&1u32).
    // `second` is declared second (position 1) but pinned to slot 0:
    //   data written as 22 must be found at child(&0u32).
    // `third` is declared third with no pin → positional slot 2:
    //   data written as 33 must be found at child(&2u32).
    let verify = env.transact(|tx| {
        tx.call(&addr.get()).run(AssertUnwindSafe(|| {
            let root = Keypath::default().child("SlotReorder");

            let val_at_slot1 = <Value<u64>>::from_keypath(root.child(&1u32));
            assert_eq!(
                val_at_slot1.get(),
                11,
                "`first` (slot 1) should hold 11"
            );

            let val_at_slot0 = <Value<u64>>::from_keypath(root.child(&0u32));
            assert_eq!(
                val_at_slot0.get(),
                22,
                "`second` (slot 0) should hold 22"
            );

            let val_at_slot2 = <Value<u64>>::from_keypath(root.child(&2u32));
            assert_eq!(
                val_at_slot2.get(),
                33,
                "`third` (slot 2) should hold 33"
            );
        }))
    });
    verify.assert_success();
}

/// Verify via keypath identity (no I/O) that each field's stored keypath
/// matches its *pinned* slot index, not its declaration index.
#[test]
fn keypath_identity_matches_pinned_slot() {
    // Keypaths hold live VM hash streams, so identity checks need a frame.
    TestEnvironment::no_cost()
        .transact(|tx| {
            tx.call(&ContractAddress::named("identity"))
                .run(|| keypath_identity_matches_pinned_slot_body())
        })
        .assert_success();
}

fn keypath_identity_matches_pinned_slot_body() {
    let root = Keypath::default().child("SlotReorder");

    // Build SlotReorder directly from its root keypath.
    let state = <SlotReorder>::from_keypath(root.clone());

    // `first` is at declaration position 0 but pinned to slot 1.
    assert_eq!(
        *state.first.key(),
        root.child(&1u32).generate_key(),
        "`first` should address slot 1 (its pinned index)"
    );
    assert_ne!(
        *state.first.key(),
        root.child(&0u32).generate_key(),
        "`first` must NOT address slot 0 (its declaration position)"
    );

    // `second` is at declaration position 1 but pinned to slot 0.
    assert_eq!(
        *state.second.key(),
        root.child(&0u32).generate_key(),
        "`second` should address slot 0 (its pinned index)"
    );
    assert_ne!(
        *state.second.key(),
        root.child(&1u32).generate_key(),
        "`second` must NOT address slot 1 (its declaration position)"
    );

    // `third` is at declaration position 2 with no pin → positional slot 2.
    assert_eq!(
        *state.third.key(),
        root.child(&2u32).generate_key(),
        "`third` should address slot 2 (positional)"
    );
}

/// Keypath-identity (no I/O): const-framed field derivation must equal
/// runtime child() at every CXS index width.
#[test]
fn wide_slot_indices_match_runtime_child() {
    // Keypaths hold live VM hash streams, so identity checks need a frame.
    TestEnvironment::no_cost()
        .transact(|tx| {
            tx.call(&ContractAddress::named("wide"))
                .run(|| wide_slot_indices_match_runtime_child_body())
        })
        .assert_success();
}

fn wide_slot_indices_match_runtime_child_body() {
    let root = Keypath::default().child("WideSlots");
    let state = <WideSlots>::from_keypath(root.clone());
    assert_eq!(*state.a.key(), root.child(&63u32).generate_key());
    assert_eq!(*state.b.key(), root.child(&300u32).generate_key());
    assert_eq!(*state.c.key(), root.child(&70_000u32).generate_key());
    assert_eq!(*state.d.key(), root.child(&u32::MAX).generate_key());
}
