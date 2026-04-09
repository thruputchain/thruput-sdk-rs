#![cfg_attr(not(feature = "dev"), no_std)]
#![allow(clippy::new_without_default)]
#![allow(dead_code)]

mod request;
pub use request::*;

use sdk::storage::*;
use sdk::*;

sdk::declare_mock_contract!();

/// State for the `Swap` contract.
#[storage]
pub struct Swap {
    requests: RCQ<SwapRequest>,
    reserves: Value<Reserves>,
}

/// A part of the `Swap` contract's state that must be updated
/// non-commutatively after processing a swap request.
#[derive(Default, ReadXDR, WriteXDR)]
pub struct Reserves {
    balance0: u32,
    balance1: u32,
}

pub const MAX_BATCH_SIZE: u32 = 3;

#[contract]
impl Swap {
    /// Submit a swap request.  Many requests may be submitted per block.
    pub fn submit_request(&mut self, r: SwapRequest) {
        self.requests.insert(&r);
    }

    /// Process some (or all) accumulated requests.
    pub fn process_requests(&mut self) -> (u32, RequestID) {
        assert!(self.caller_is_administrator());

        let mut n_processed = 0;
        let mut id_extent = 0;
        let mut reserves = self.reserves.try_get().unwrap_or_default();

        while let Some(req) = self.requests.try_consume() {
            // Keep track of the highest request id
            id_extent = id_extent.max(req.id);

            // Simulate pending update of swap state
            reserves.balance0 += 1;
            reserves.balance1 += 1;

            n_processed += 1;
            if n_processed == MAX_BATCH_SIZE {
                break;
            }
        }

        // Simulate non-commutative update of swap state after processing the batch
        self.reserves.set(&reserves);

        (n_processed, id_extent)
    }

    fn caller_is_administrator(&self) -> bool {
        let _ = get_caller();

        true
    }
}
