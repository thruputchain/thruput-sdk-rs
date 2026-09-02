//! Time and block-height arithmetic against the real host: deadlines built
//! from the block timepoint, durations across pinned blocks, and timepoints
//! travelling through storage.

use sdk::Named;
use sdk::contract_test;
use sdk::storage::{Keypath, Value};
use sdk::time::{DAY, Duration, HOUR, MINUTE, WEEK};
use sdk::{BlockHeight, TimePoint};

contract_test!(deadline_reads_like_solidity {
    let now = sdk::block::get_block_timepoint();
    let deadline = &now + 7 * DAY;

    assert!(now < deadline);
    assert!(deadline > now);
    assert_eq!(&deadline - &now, WEEK);
    assert_eq!(&deadline - WEEK, now);
    assert_eq!(&now - &deadline, Duration::ZERO);
    assert_eq!(now.checked_duration_since(&deadline), None);
});

contract_test!(expiry_height_from_block_height {
    let now = sdk::block::get_block_height();
    let expiry = &now + 100;

    assert!(now < expiry);
    assert_eq!(&expiry - &now, 100);
    assert_eq!(&expiry - 100, now);
    assert_eq!(now.blocks_since(&expiry), -100);
});

contract_test!(deadlines_roundtrip_through_storage {
    let now = sdk::block::get_block_timepoint();
    let mut deadline: Value<TimePoint> = Keypath::default().into();
    deadline.set(&(&now + HOUR));
    assert_eq!(deadline.get() - &now, HOUR);

    let height = sdk::block::get_block_height();
    let mut expiry: Value<BlockHeight> = Keypath::default().into();
    expiry.set(&(&height + 3));
    assert_eq!(expiry.get() - &height, 3);
});

fn read_block_timepoint(
    env: &sdk::TestEnvironmentHandle,
    address: &sdk::ContractAddress,
) -> TimePoint {
    let result = env
        .transact(|tx| tx.call(address).run(sdk::block::get_block_timepoint));
    result.assert_success();
    result.assert_return_as::<TimePoint>()
}

#[test]
fn pinned_blocks_are_a_duration_apart() {
    let env = sdk::TestEnvironment::no_cost();
    let address = sdk::ContractAddress::named("time_pinned_blocks");

    let first = read_block_timepoint(&env, &address);
    env.advance_block_at(&first + HOUR);
    let second = read_block_timepoint(&env, &address);
    assert_eq!(&second - &first, HOUR);

    env.advance_block_at(&second + 90 * MINUTE);
    let third = read_block_timepoint(&env, &address);
    assert_eq!(&third - &first, Duration::from_mins(150));
    assert_eq!(third.duration_since(&second), Duration::from_secs(5_400));
}
