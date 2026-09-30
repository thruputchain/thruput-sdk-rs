#![cfg_attr(not(feature = "dev"), no_std)]

// The same crate and trait names as 0.1, with a wider value type in 0.2.
#[sdk::contract]
pub trait Adder {
    fn add_one(value: u64) -> u64;
}
