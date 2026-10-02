//! `get_address_auth_summary` and `get_address_public_key`, exercised against
//! the real host: the executing contract's own key registration, read back
//! as the current frame sees it.

use sdk::types::{CryptographicScheme, Ed25519PublicKey, PublicKey};
use sdk::{AddressAuthSummary, Named, contract_test};

fn key(byte: u8) -> PublicKey {
    PublicKey::Ed25519 {
        ed25519: Ed25519PublicKey::from([byte; 32]),
    }
}

/// Every slot except those in `occupied` is empty.
fn assert_slots_empty_except(
    summary: &AddressAuthSummary,
    occupied: &[usize],
) {
    for (index, slot) in summary.public_keys.iter().enumerate() {
        assert_eq!(
            slot.is_some(),
            occupied.contains(&index),
            "slot {index}: {slot:?}"
        );
    }
}

/// Slot `index` reports an Ed25519 key at `max_weight`.
fn assert_slot(summary: &AddressAuthSummary, index: usize, max_weight: u32) {
    let slot = summary.public_keys[index]
        .as_ref()
        .unwrap_or_else(|| panic!("slot {index} is empty"));
    assert_eq!(slot.scheme, CryptographicScheme::Ed25519);
    assert_eq!(slot.max_weight, max_weight);
}

/// Slot `index` reads back as `expected` through the per-key syscall.
fn assert_key(index: u32, expected: &PublicKey) {
    let read =
        sdk::get_address_public_key(index, CryptographicScheme::Ed25519)
            .unwrap_or_else(|e| panic!("slot {index}: {e:?}"));
    assert_eq!(&read, expected, "slot {index}");
}

fn assert_key_absent(index: u32) {
    let err = sdk::get_address_public_key(index, CryptographicScheme::Ed25519)
        .expect_err("empty slot reads as an error");
    assert!(err.is_storage_key_not_exist(), "slot {index}: {err:?}");
    assert_eq!(
        sdk::try_get_address_public_key(index, CryptographicScheme::Ed25519)
            .expect("try_get on an empty slot"),
        None
    );
}

contract_test!(fresh_address_reads_an_empty_registration {
    // No row yet: not an error, but an all-empty registration, and every
    // slot's key is absent.
    let summary = sdk::get_address_auth_summary().expect("summary");
    assert_slots_empty_except(&summary, &[]);
    assert_eq!(summary.required_weight, 0);
    assert_eq!(summary.last_written_height.clone().into_inner(), 0);
    assert_key_absent(0);
    assert_key_absent(20);
});

contract_test!(slot_past_the_table_is_rejected {
    let err = sdk::get_address_public_key(21, CryptographicScheme::Ed25519)
        .expect_err("slot 21 does not exist");
    assert!(err.is_invalid_input(), "{err:?}");
});

contract_test!(same_frame_registration_is_visible {
    sdk::add_address_public_key(3, key(7), 2).expect("register slot 3");
    sdk::set_address_required_weight(2).expect("set required weight");

    // The pending writes are part of the frame's view, stamped with the
    // executing height the row will carry once the block commits.
    let summary = sdk::get_address_auth_summary().expect("summary");
    assert_slot(&summary, 3, 2);
    assert_slots_empty_except(&summary, &[3]);
    assert_eq!(summary.required_weight, 2);
    assert_eq!(summary.last_written_height, sdk::block::get_block_height());
    assert_key(3, &key(7));
    assert_key_absent(2);
});

contract_test!(clearing_a_slot_of_a_fresh_address_stays_empty {
    // A clear of a slot that was never held writes no row, and the reads
    // agree: still empty, still unstamped.
    sdk::remove_address_public_key(0).expect("remove slot 0");
    let summary = sdk::get_address_auth_summary().expect("summary");
    assert_slots_empty_except(&summary, &[]);
    assert_eq!(summary.required_weight, 0);
    assert_eq!(summary.last_written_height.clone().into_inner(), 0);
    assert_key_absent(0);
});

// The acceptance scenario across blocks: what one transaction registers, a
// later block's transaction reads back from the committed row; a removal in
// that transaction clears the slot at once while the committed weight stays.
#[test]
fn committed_registration_reads_back_across_blocks() {
    let env = sdk::TestEnvironment::no_cost();
    let address = sdk::ContractAddress::named("address_auth_summary_blocks");

    env.transact(|tx| {
        tx.call(&address).run(|| {
            sdk::add_address_public_key(3, key(7), 2)
                .expect("register slot 3");
            sdk::set_address_required_weight(2).expect("set required weight");
        })
    })
    .assert_success();
    env.advance_block();

    env.transact(|tx| {
        tx.call(&address).run(|| {
            let committed = sdk::get_address_auth_summary().expect("summary");
            assert_slot(&committed, 3, 2);
            assert_slots_empty_except(&committed, &[3]);
            assert_eq!(committed.required_weight, 2);
            assert!(
                committed.last_written_height < sdk::block::get_block_height(),
                "committed row is stamped with an earlier block"
            );
            assert_key(3, &key(7));

            // A second key layers over the committed row without disturbing
            // slot 3; removing slot 3 then clears it, weight intact.
            sdk::add_address_public_key(1, key(9), 1)
                .expect("register slot 1");
            let layered = sdk::get_address_auth_summary().expect("summary");
            assert_slot(&layered, 1, 1);
            assert_slot(&layered, 3, 2);
            assert_slots_empty_except(&layered, &[1, 3]);
            assert_eq!(layered.required_weight, 2);
            assert_eq!(
                layered.last_written_height,
                sdk::block::get_block_height()
            );
            assert_key(1, &key(9));
            assert_key(3, &key(7));

            sdk::remove_address_public_key(3).expect("remove slot 3");
            let cleared = sdk::get_address_auth_summary().expect("summary");
            assert_slot(&cleared, 1, 1);
            assert_slots_empty_except(&cleared, &[1]);
            assert_eq!(cleared.required_weight, 2);
            assert_key_absent(3);
        })
    })
    .assert_success();
    env.advance_block();

    env.transact(|tx| {
        tx.call(&address).run(|| {
            let summary = sdk::get_address_auth_summary().expect("summary");
            assert_slot(&summary, 1, 1);
            assert_slots_empty_except(&summary, &[1]);
            assert_eq!(summary.required_weight, 2);
            assert_key(1, &key(9));
            assert_key_absent(3);
        })
    })
    .assert_success();
}
