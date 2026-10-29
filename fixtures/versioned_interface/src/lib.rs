#![cfg_attr(not(feature = "dev"), no_std)]

// The interface version comes from this crate's Cargo.toml: 0.1.0.
#[thruput_sdk::contract]
pub trait Adder {
    fn add_one(value: u32) -> u32;
}
