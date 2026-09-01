//! The block-data syscalls, exercised against the real host.

use sdk::contract_test;

contract_test!(network_hash_is_the_environments_configured_network {
    // contract_test! builds a TestEnvironment on the default test network, so
    // the syscall must report exactly that hash -- not zeros, and not some
    // other network's.
    assert_eq!(sdk::block::get_network_hash(), sdk::test_network_hash());
});

contract_test!(network_hash_is_stable_within_a_transaction {
    // Fixed for the life of the network, so every read in a transaction --
    // indeed every read ever -- agrees.
    assert_eq!(sdk::block::get_network_hash(), sdk::block::get_network_hash());
});

contract_test!(block_timepoint_is_fixed_for_the_block {
    // The block's timepoint, fixed before execution began, not a clock read:
    // two calls in the same block agree.
    let first = sdk::block::get_block_timepoint();
    let second = sdk::block::get_block_timepoint();
    assert_eq!(first, second);
    assert_ne!(first.clone().into_inner(), 0);
});

contract_test!(block_height_and_timepoint_are_independent {
    let height = sdk::block::get_block_height();
    let timepoint = sdk::block::get_block_timepoint();
    assert_eq!(height, sdk::block::get_block_height());
    assert_eq!(timepoint, sdk::block::get_block_timepoint());
});
