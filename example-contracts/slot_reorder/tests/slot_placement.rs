use std::cell::Cell;
use std::panic::AssertUnwindSafe;

use slot_reorder::*;

static NONCE: SHA256Hash = sdk::sha256_static!("SLOT_REORDER_NONCE");

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
    //   Keypath::root().child("SlotReorder")
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
            let root = Keypath::root().child("SlotReorder");

            let val_at_slot1: Value<u64> = root.child(&1u32).into();
            assert_eq!(
                val_at_slot1.get(),
                11,
                "`first` (slot 1) should hold 11"
            );

            let val_at_slot0: Value<u64> = root.child(&0u32).into();
            assert_eq!(
                val_at_slot0.get(),
                22,
                "`second` (slot 0) should hold 22"
            );

            let val_at_slot2: Value<u64> = root.child(&2u32).into();
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
    use sdk::storage::StorageDomain;

    let domain = <SlotReorder as StorageDomain>::DOMAIN;
    assert_eq!(domain, "SlotReorder");

    let root = Keypath::root().child(domain);

    // Build SlotReorder directly from its root keypath.
    let state = SlotReorder::from(root.clone());

    // `first` is at declaration position 0 but pinned to slot 1.
    assert!(
        state
            .first
            .keypath()
            .addresses_same_slot(&root.child(&1u32)),
        "`first` should address slot 1 (its pinned index)"
    );
    assert!(
        !state
            .first
            .keypath()
            .addresses_same_slot(&root.child(&0u32)),
        "`first` must NOT address slot 0 (its declaration position)"
    );

    // `second` is at declaration position 1 but pinned to slot 0.
    assert!(
        state
            .second
            .keypath()
            .addresses_same_slot(&root.child(&0u32)),
        "`second` should address slot 0 (its pinned index)"
    );
    assert!(
        !state
            .second
            .keypath()
            .addresses_same_slot(&root.child(&1u32)),
        "`second` must NOT address slot 1 (its declaration position)"
    );

    // `third` is at declaration position 2 with no pin → positional slot 2.
    assert!(
        state
            .third
            .keypath()
            .addresses_same_slot(&root.child(&2u32)),
        "`third` should address slot 2 (positional)"
    );
}
