#![cfg_attr(not(feature = "dev"), no_std)]

thruput_sdk::declare_mock_contract!();

pub use thruput_sdk::storage::*;
pub use thruput_sdk::*;

#[thruput_sdk::storage(domain = "Shared")]
pub struct Alpha { v: Value<u64> }

#[thruput_sdk::storage(domain = "Shared")]   // deliberate collision
pub struct Beta { v: Value<u64> }

#[contract]
impl Alpha { pub fn a(&self) -> u64 { self.v.get() } }

#[contract]
impl Beta { pub fn b(&self) -> u64 { self.v.get() } }
