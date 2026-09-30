#![cfg_attr(not(feature = "dev"), no_std)]

use adder_v1::Adder as AdderV1;
use adder_v2::Adder as AdderV2;

sdk::declare_mock_contract!();

pub struct Contract;

// This crate is 0.2.0, but the implemented interface stays at 0.1.0.
#[sdk::contract]
impl AdderV1 for Contract {
    fn add_one(value: u32) -> u32 {
        value + 1
    }
}

// Both crates declare Adder::add_one, but the 0.2 interface takes u64.
// Its distinct version gives it a separate entrypoint in the same contract.
#[sdk::contract]
impl AdderV2 for Contract {
    fn add_one(value: u64) -> u64 {
        value + 1
    }
}

#[cfg(feature = "bottle-test")]
mod bottle_test;
