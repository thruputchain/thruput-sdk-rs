//! The block-data syscalls, exercised against the real host.

use thruput_sdk::contract_test;
use thruput_sdk::{Duration, Named, TimePoint};

contract_test!(network_hash_is_the_environments_configured_network {
    // contract_test! builds a TestEnvironment on the default test network, so
    // the syscall must report exactly that hash -- not zeros, and not some
    // other network's.
    assert_eq!(thruput_sdk::block::get_network_hash(), thruput_sdk::test_network_hash());
});

contract_test!(network_hash_is_stable_within_a_transaction {
    // Fixed for the life of the network, so every read in a transaction --
    // indeed every read ever -- agrees.
    assert_eq!(thruput_sdk::block::get_network_hash(), thruput_sdk::block::get_network_hash());
});

contract_test!(block_timepoint_is_fixed_for_the_block {
    // The block's timepoint, fixed before execution began, not a clock read:
    // two calls in the same block agree.
    let first = thruput_sdk::block::get_block_timepoint();
    let second = thruput_sdk::block::get_block_timepoint();
    assert_eq!(first, second);
    assert_ne!(first.clone().into_inner(), 0);
});

contract_test!(block_height_and_timepoint_are_independent {
    let height = thruput_sdk::block::get_block_height();
    let timepoint = thruput_sdk::block::get_block_timepoint();
    assert_eq!(height, thruput_sdk::block::get_block_height());
    assert_eq!(timepoint, thruput_sdk::block::get_block_timepoint());
});

// A single contract_test! runs one transaction in one block, so the properties
// that only show up across blocks need the environment driven directly. The
// contract body returns the timepoint rather than capturing it: `run` takes an
// UnwindSafe closure, which `&mut` capture is not.

/// The wall clock, as the timepoint the host would stamp a block with now.
fn wall_clock() -> TimePoint {
    TimePoint::UNIX_EPOCH
        + std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock is before the Unix epoch")
}

fn read_block_timepoint(
    env: &thruput_sdk::TestEnvironmentHandle,
    address: &thruput_sdk::ContractAddress,
) -> TimePoint {
    let result = env.transact(|tx| {
        tx.call(address)
            .run(thruput_sdk::block::get_block_timepoint)
    });
    result.assert_success();
    result.assert_return_as::<thruput_sdk::TimePoint>()
}

#[test]
fn block_timepoint_defaults_to_a_real_clock_reading() {
    // Before the environment exists: it stamps its first block's timepoint at
    // construction, so a bound taken afterwards is already too late.
    let before = wall_clock();

    let env = thruput_sdk::TestEnvironment::no_cost();
    let address =
        thruput_sdk::ContractAddress::named("block_timepoint_wall_clock");

    let first = read_block_timepoint(&env, &address);
    env.advance_block();
    let second = read_block_timepoint(&env, &address);
    let after = wall_clock();

    // Bracketed by clock reads taken around the test. This is what separates a
    // real Unix-nanosecond reading from the constant the host used to report:
    // 1'700'000'000 is a Unix *seconds* value in a nanoseconds field, so it
    // sits about fifty years below `before`.
    let window = before.clone()..=after.clone();
    for observed in [&first, &second] {
        assert!(
            window.contains(observed),
            "timepoint {} outside [{}, {}]",
            **observed,
            *before,
            *after
        );
    }

    // Only that blocks do not move backwards -- equal timepoints are legal, so
    // asserting strict progress here would be testing the clock's granularity
    // rather than the contract. That successive blocks carry *distinct*
    // timepoints when asked to is covered deterministically by
    // advance_block_at_pins_the_timepoint.
    assert!(
        second >= first,
        "timepoint went backwards: {} < {}",
        *second,
        *first
    );
}

#[test]
fn advance_block_at_pins_the_timepoint() {
    let env = thruput_sdk::TestEnvironment::no_cost();
    let address =
        thruput_sdk::ContractAddress::named("block_timepoint_pinned");

    // Pin forward from where we are: the environment opens its first block on
    // the wall clock, and blocks may not move backwards.
    const ONE_HOUR: Duration = Duration::from_hours(1);
    let pinned = &read_block_timepoint(&env, &address) + ONE_HOUR;

    env.advance_block_at(pinned.clone());
    assert_eq!(read_block_timepoint(&env, &address), pinned);
    // Still fixed for the whole block.
    assert_eq!(read_block_timepoint(&env, &address), pinned);

    let later = &pinned + ONE_HOUR;
    env.advance_block_at(later.clone());
    assert_eq!(read_block_timepoint(&env, &address), later);
}

#[test]
#[should_panic(expected = "precedes the block it would follow")]
fn advance_block_at_refuses_to_go_backwards() {
    let env = thruput_sdk::TestEnvironment::no_cost();
    let address =
        thruput_sdk::ContractAddress::named("block_timepoint_backwards");

    let now = read_block_timepoint(&env, &address);
    env.advance_block_at(&now - Duration::from_nanos(1));
}

#[test]
fn later_blocks_continue_from_a_pinned_timepoint() {
    let env = thruput_sdk::TestEnvironment::no_cost();
    let address =
        thruput_sdk::ContractAddress::named("block_timepoint_continues");

    // Pin three days ahead; a plain advance should continue from there.
    const THREE_DAYS: Duration = Duration::from_hours(72);
    let pinned = &read_block_timepoint(&env, &address) + THREE_DAYS;
    env.advance_block_at(pinned.clone());

    env.advance_block();
    let next = read_block_timepoint(&env, &address);
    assert!(next >= pinned, "{} < {}", *next, *pinned);
    assert!(
        next < &pinned + Duration::from_hours(1),
        "{} jumped past {}",
        *next,
        *pinned
    );

    env.advance_block();
    assert!(read_block_timepoint(&env, &address) >= next);
}

#[test]
fn a_zero_step_freezes_the_clock_at_a_pinned_timepoint() {
    let env = thruput_sdk::TestEnvironment::no_cost();
    let address = thruput_sdk::ContractAddress::named("block_step_frozen");

    let tick = &read_block_timepoint(&env, &address) + Duration::from_hours(1);
    env.advance_block_at(tick.clone());
    env.set_block_time_step(Some(Duration::ZERO));
    for _ in 0..3 {
        std::thread::sleep(std::time::Duration::from_millis(2));
        env.advance_block();
        assert_eq!(read_block_timepoint(&env, &address), tick);
    }
}

#[test]
fn a_step_spaces_blocks_exactly() {
    let env = thruput_sdk::TestEnvironment::no_cost();
    let address = thruput_sdk::ContractAddress::named("block_step_spaced");

    const STEP: Duration = Duration::from_secs(12);
    let pinned =
        &read_block_timepoint(&env, &address) + Duration::from_hours(1);
    env.advance_block_at(pinned.clone());
    env.set_block_time_step(Some(STEP));
    env.advance_block();
    assert_eq!(read_block_timepoint(&env, &address), &pinned + STEP);
    env.advance_block();
    assert_eq!(read_block_timepoint(&env, &address), &pinned + 2 * STEP);

    // A pin still wins, and stepping continues from it.
    let repinned = &pinned + Duration::from_hours(24);
    env.advance_block_at(repinned.clone());
    env.advance_block();
    assert_eq!(read_block_timepoint(&env, &address), &repinned + STEP);
}

#[test]
fn without_a_pin_the_first_stepped_block_takes_the_wall_clock() {
    let env = thruput_sdk::TestEnvironment::no_cost();
    let address = thruput_sdk::ContractAddress::named("block_step_unpinned");

    let before = wall_clock();
    env.set_block_time_step(Some(Duration::ZERO));
    env.advance_block();
    let first = read_block_timepoint(&env, &address);
    let after = wall_clock();
    assert!(
        (before.clone()..=after.clone()).contains(&first),
        "timepoint {} outside [{}, {}]",
        *first,
        *before,
        *after
    );

    std::thread::sleep(std::time::Duration::from_millis(2));
    env.advance_block();
    assert_eq!(read_block_timepoint(&env, &address), first, "then frozen");
}

#[test]
fn clearing_the_step_continues_from_the_last_block() {
    let env = thruput_sdk::TestEnvironment::no_cost();
    let address = thruput_sdk::ContractAddress::named("block_step_cleared");

    const THREE_DAYS: Duration = Duration::from_hours(72);
    let pinned = &read_block_timepoint(&env, &address) + THREE_DAYS;
    env.advance_block_at(pinned.clone());
    env.set_block_time_step(Some(Duration::ZERO));
    env.advance_block();
    env.set_block_time_step(None);

    env.advance_block();
    let next = read_block_timepoint(&env, &address);
    assert!(next >= pinned, "{} < {}", *next, *pinned);
    assert!(
        next < &pinned + Duration::from_hours(1),
        "{} jumped back toward today or past {}",
        *next,
        *pinned
    );
}

#[test]
#[should_panic(expected = "does not fit in u64 nanoseconds")]
fn a_step_beyond_u64_nanoseconds_panics() {
    let env = thruput_sdk::TestEnvironment::no_cost();
    env.set_block_time_step(Some(Duration::MAX));
}
