// `TimePoint`, `Duration`, and `BlockCount` in every position a contract
// type can occupy: method arguments, return values, and `#[contract_type]`
// fields. Compiling at all is the test -- each position needs the spec,
// auth-handle, and mintable registrations behind it.

#![allow(unexpected_cfgs)]

use sdk::block::BlockCount;
use sdk::storage::Value;
use sdk::time::Duration;
use sdk::*;

#[contract_type]
pub struct Vesting {
    pub start: TimePoint,
    pub cliff: Duration,
    pub review_every: BlockCount,
}

#[sdk::storage]
pub struct Schedule {
    vesting: Value<Vesting>,
}

#[contract]
impl Schedule {
    pub fn configure(
        &mut self,
        start: TimePoint,
        cliff: Duration,
        review_every: BlockCount,
    ) {
        self.vesting.set(&Vesting { start, cliff, review_every });
    }

    pub fn cliff_ends(&self) -> TimePoint {
        let vesting = self.vesting.get();
        vesting.start + vesting.cliff
    }

    pub fn next_review(&self) -> BlockHeight {
        sdk::block::get_block_height() + self.vesting.get().review_every
    }

    pub fn remaining(&self) -> Duration {
        self.cliff_ends().duration_since(&sdk::block::get_block_timepoint())
    }

    pub fn vesting(&self) -> Vesting {
        self.vesting.get()
    }
}

fn main() {
    let _client = ScheduleClient::new(&ContractAddress::default());
}
