#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();

use sdk::*;

pub struct VersionedCaller;

#[contract(client = VersionedCallerClient)]
impl VersionedCaller {
    pub fn call_add_one(target: &ContractAddress, x: u32) -> u32 {
        versioned_callee::VersionedCalleeClient::new(target).add_one(x)
    }
}
