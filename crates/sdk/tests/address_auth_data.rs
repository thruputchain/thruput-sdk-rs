//! `get_address_auth_data`, exercised against the real host: the executing
//! contract's own key registration, read back as the current frame sees it.

use sdk::types::{Ed25519PublicKey, PublicKey};
use sdk::{AddressAuthData, Named, contract_test};

fn key(byte: u8) -> PublicKey {
    PublicKey::Ed25519 {
        ed25519: Ed25519PublicKey::from([byte; 32]),
    }
}

/// Every slot except those in `occupied` is empty.
fn assert_slots_empty_except(auth_data: &AddressAuthData, occupied: &[usize]) {
    for (index, slot) in auth_data.public_keys.iter().enumerate() {
        assert_eq!(
            slot.is_some(),
            occupied.contains(&index),
            "slot {index}: {slot:?}"
        );
    }
}

fn assert_slot(
    auth_data: &AddressAuthData,
    index: usize,
    public_key: &PublicKey,
    max_weight: u32,
) {
    let slot = auth_data.public_keys[index]
        .as_ref()
        .unwrap_or_else(|| panic!("slot {index} is empty"));
    assert_eq!(&slot.key, public_key);
    assert_eq!(slot.max_weight, max_weight);
}

contract_test!(fresh_address_reads_an_empty_registration {
    // No row yet: not an error, but an all-empty registration.
    let auth_data = sdk::get_address_auth_data().expect("GetAddressAuthData");
    assert_slots_empty_except(&auth_data, &[]);
    assert_eq!(auth_data.required_weight, 0);
    assert_eq!(auth_data.last_written_height.clone().into_inner(), 0);
});

contract_test!(same_frame_registration_is_visible {
    sdk::add_address_public_key(3, key(7), 2).expect("register slot 3");
    sdk::set_address_required_weight(2).expect("set required weight");

    // The pending writes are part of the frame's view, stamped with the
    // executing height the row will carry once the block commits.
    let auth_data = sdk::get_address_auth_data().expect("GetAddressAuthData");
    assert_slot(&auth_data, 3, &key(7), 2);
    assert_slots_empty_except(&auth_data, &[3]);
    assert_eq!(auth_data.required_weight, 2);
    assert_eq!(auth_data.last_written_height, sdk::block::get_block_height());
});

contract_test!(clearing_a_slot_of_a_fresh_address_stays_empty {
    // A clear of a slot that was never held writes no row, and the read
    // agrees: still empty, still unstamped.
    sdk::remove_address_public_key(0).expect("remove slot 0");
    let auth_data = sdk::get_address_auth_data().expect("GetAddressAuthData");
    assert_slots_empty_except(&auth_data, &[]);
    assert_eq!(auth_data.required_weight, 0);
    assert_eq!(auth_data.last_written_height.clone().into_inner(), 0);
});

// The acceptance scenario across blocks: what one transaction registers, a
// later block's transaction reads back from the committed row; a removal in
// that transaction clears the slot at once while the committed weight stays.
#[test]
fn committed_registration_reads_back_across_blocks() {
    let env = sdk::TestEnvironment::no_cost();
    let address =
        sdk::ContractAddress::named("address_auth_data_across_blocks");

    env.transact(|tx| {
        tx.call(&address).run(|| {
            sdk::add_address_public_key(3, key(7), 2)
                .expect("register slot 3");
            sdk::set_address_required_weight(2).expect("set required weight");
            sdk::block::get_block_height()
        })
    })
    .assert_success();
    env.advance_block();

    env.transact(|tx| {
        tx.call(&address).run(|| {
            let committed =
                sdk::get_address_auth_data().expect("GetAddressAuthData");
            assert_slot(&committed, 3, &key(7), 2);
            assert_slots_empty_except(&committed, &[3]);
            assert_eq!(committed.required_weight, 2);
            assert!(
                committed.last_written_height < sdk::block::get_block_height(),
                "committed row is stamped with an earlier block"
            );

            // A second key layers over the committed row without disturbing
            // slot 3; removing slot 3 then clears it, weight intact.
            sdk::add_address_public_key(1, key(9), 1)
                .expect("register slot 1");
            let layered =
                sdk::get_address_auth_data().expect("GetAddressAuthData");
            assert_slot(&layered, 1, &key(9), 1);
            assert_slot(&layered, 3, &key(7), 2);
            assert_slots_empty_except(&layered, &[1, 3]);
            assert_eq!(layered.required_weight, 2);
            assert_eq!(
                layered.last_written_height,
                sdk::block::get_block_height()
            );

            sdk::remove_address_public_key(3).expect("remove slot 3");
            let cleared =
                sdk::get_address_auth_data().expect("GetAddressAuthData");
            assert_slot(&cleared, 1, &key(9), 1);
            assert_slots_empty_except(&cleared, &[1]);
            assert_eq!(cleared.required_weight, 2);
        })
    })
    .assert_success();
    env.advance_block();

    env.transact(|tx| {
        tx.call(&address).run(|| {
            let auth_data =
                sdk::get_address_auth_data().expect("GetAddressAuthData");
            assert_slot(&auth_data, 1, &key(9), 1);
            assert_slots_empty_except(&auth_data, &[1]);
            assert_eq!(auth_data.required_weight, 2);
        })
    })
    .assert_success();
}
