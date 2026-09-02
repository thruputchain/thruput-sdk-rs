// `TimePoint` in every position a contract type can occupy: method
// arguments, return values, and `#[contract_type]` fields, alongside the
// arithmetic on it and on `BlockHeight`. Compiling at all is the test --
// each position needs the spec, auth-handle, and mintable registrations
// behind it.

#![allow(unexpected_cfgs)]

use sdk::storage::Value;
use sdk::time::{DAY, Duration};
use sdk::*;

#[contract_type]
pub struct Vesting {
    pub start: TimePoint,
    pub cliff_ends: TimePoint,
    pub review_at: BlockHeight,
}

#[sdk::storage]
pub struct Schedule {
    vesting: Value<Vesting>,
}

#[contract]
impl Schedule {
    pub fn configure(&mut self, start: TimePoint, review_after: i64) {
        self.vesting.set(&Vesting {
            cliff_ends: &start + 90 * DAY,
            start,
            review_at: sdk::block::get_block_height() + review_after,
        });
    }

    pub fn cliff_ends(&self) -> TimePoint {
        self.vesting.get().cliff_ends
    }

    pub fn remaining_nanos(&self) -> u64 {
        let remaining: Duration =
            self.cliff_ends() - sdk::block::get_block_timepoint();
        remaining.as_nanos() as u64
    }

    pub fn blocks_until_review(&self) -> i64 {
        self.vesting.get().review_at - sdk::block::get_block_height()
    }

    pub fn vesting(&self) -> Vesting {
        self.vesting.get()
    }
}

fn main() {
    let _client = ScheduleClient::new(&ContractAddress::default());
}
