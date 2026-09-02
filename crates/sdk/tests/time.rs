//! Time and block-height arithmetic against the real host: deadlines built
//! from the block timepoint, durations across pinned blocks, and the new
//! types travelling through storage.

use sdk::Named;
use sdk::block::BlockCount;
use sdk::contract_test;
use sdk::storage::{Keypath, Value};
use sdk::time::Duration;
use sdk::{BlockHeightExt, TimePoint, TimePointExt};

contract_test!(deadline_reads_like_solidity {
    let now = sdk::block::get_block_timepoint();
    let deadline = &now + 7 * Duration::DAY;

    assert!(now < deadline);
    assert!(deadline > now);
    assert_eq!(deadline.duration_since(&now), Duration::from_days(7));
    assert_eq!(&deadline - Duration::WEEK, now);
    assert_eq!(now.duration_since(&deadline), Duration::ZERO);
    assert_eq!(now.checked_duration_since(&deadline), None);
});

contract_test!(expiry_height_from_block_height {
    let now = sdk::block::get_block_height();
    let expiry = &now + BlockCount::new(100);

    assert!(now < expiry);
    assert_eq!(expiry.blocks_since(&now), BlockCount::new(100));
    assert_eq!(&expiry - BlockCount::new(100), now);
});

contract_test!(durations_and_counts_roundtrip_through_storage {
    let mut lockup: Value<Duration> = Keypath::default().into();
    lockup.set(&Duration::from_hours(36));
    assert_eq!(lockup.get(), Duration::DAY + Duration::from_hours(12));

    let mut period: Value<BlockCount> = Keypath::default().into();
    period.set(&BlockCount::new(-3));
    assert_eq!(period.get(), BlockCount::new(-3));

    let mut deadline: Value<TimePoint> = Keypath::default().into();
    let now = sdk::block::get_block_timepoint();
    deadline.set(&(&now + Duration::HOUR));
    assert_eq!(deadline.get().duration_since(&now), Duration::HOUR);
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
    env.advance_block_at(&first + Duration::HOUR);
    let second = read_block_timepoint(&env, &address);
    assert_eq!(second.duration_since(&first), Duration::HOUR);

    env.advance_block_at(&second + 90 * Duration::MINUTE);
    let third = read_block_timepoint(&env, &address);
    assert_eq!(third.duration_since(&first), Duration::from_mins(150));
    assert_eq!(third.duration_since(&second), Duration::from_secs(5_400));
}
