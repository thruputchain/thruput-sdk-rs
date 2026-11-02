#![cfg_attr(not(feature = "dev"), no_std)]

thruput_sdk::declare_mock_contract!();

use thruput_sdk::*;

pub struct NamedInterfaceCaller;

#[contract(client = NamedInterfaceCallerClient)]
impl NamedInterfaceCaller {
    pub fn call_add_one(target: &ContractAddress, x: u32) -> u32 {
        named_interface_callee::ExplicitNamedInterfaceClient::new(target)
            .add_one(x)
    }
}
