#![cfg_attr(not(feature = "dev"), no_std)]

use sdk::*;

#[derive(Default)]
pub struct BareImplCaller;

#[contract]
impl BareImplCaller {
    pub fn call_add_one(target: &ContractAddress, x: u32) -> u32 {
        bare_impl_callee::BareImplCalleeClient::new(target).add_one(x)
    }
}
