#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_package!();

use sdk::*;

#[derive(Default)]
pub struct BareImplCallee;

#[contract]
impl BareImplCallee {
    pub fn add_one(x: u32) -> u32 {
        x + 1
    }
}
