#![cfg_attr(not(feature = "dev"), no_std)]

use versioned_interface::Adder;

sdk::declare_mock_contract!();

pub struct Contract;

// This crate is 0.2.0, but the implemented interface stays at 0.1.0.
#[sdk::contract]
impl Adder for Contract {
    fn add_one(value: u32) -> u32 {
        value + 1
    }
}

#[cfg(feature = "bottle-test")]
mod bottle_test;
