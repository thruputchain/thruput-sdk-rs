#![cfg_attr(not(feature = "dev"), no_std)]

thruput_sdk::declare_mock_contract!();

use thruput_sdk::*;

pub struct VersionedCaller;

#[contract(client = VersionedCallerClient)]
impl VersionedCaller {
    pub fn call_add_one(target: &ContractAddress, x: u32) -> u32 {
        versioned_callee::VersionedCalleeClient::new(target).add_one(x)
    }
}
