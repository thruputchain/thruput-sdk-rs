//! The block-data syscalls, exercised against the real host.

use sdk::Named;
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

// A single contract_test! runs one transaction in one block, so the properties
// that only show up across blocks need the environment driven directly. The
// contract body returns the timepoint rather than capturing it: `run` takes an
// UnwindSafe closure, which `&mut` capture is not.

fn read_block_timepoint(
    env: &sdk::TestEnvironmentHandle,
    address: &sdk::ContractAddress,
) -> u64 {
    let result = env
        .transact(|tx| tx.call(address).run(sdk::block::get_block_timepoint));
    result.assert_success();
    result.assert_return_as::<sdk::TimePoint>().into_inner()
}

#[test]
fn block_timepoint_moves_forward_across_blocks() {
    let env = sdk::TestEnvironment::no_cost();
    let address = sdk::ContractAddress::named("block_timepoint_advances");

    let first = read_block_timepoint(&env, &address);
    env.advance_block();
    let second = read_block_timepoint(&env, &address);

    // Each block takes its own clock reading, so time moves. Before test_ffi
    // 0.28.0 the host reported one hardcoded constant for every block, and
    // these two were equal.
    assert!(
        second > first,
        "timepoint did not advance: {second} follows {first}"
    );
}

#[test]
fn advance_block_at_pins_the_timepoint() {
    let env = sdk::TestEnvironment::no_cost();
    let address = sdk::ContractAddress::named("block_timepoint_pinned");

    // Pin forward from where we are: the environment opens its first block on
    // the wall clock, and blocks may not move backwards.
    const ONE_HOUR_NANOS: u64 = 3_600_000_000_000;
    let pinned = read_block_timepoint(&env, &address) + ONE_HOUR_NANOS;

    env.advance_block_at(pinned.into());
    assert_eq!(read_block_timepoint(&env, &address), pinned);
    // Still fixed for the whole block.
    assert_eq!(read_block_timepoint(&env, &address), pinned);

    let later = pinned + ONE_HOUR_NANOS;
    env.advance_block_at(later.into());
    assert_eq!(read_block_timepoint(&env, &address), later);
}

#[test]
#[should_panic(expected = "precedes the block it would follow")]
fn advance_block_at_refuses_to_go_backwards() {
    let env = sdk::TestEnvironment::no_cost();
    let address = sdk::ContractAddress::named("block_timepoint_backwards");

    let now = read_block_timepoint(&env, &address);
    env.advance_block_at((now - 1).into());
}
