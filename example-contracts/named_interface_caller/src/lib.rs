#![cfg_attr(not(feature = "dev"), no_std)]

use sdk::*;

pub struct NamedInterfaceCaller;

#[contract(client = NamedInterfaceCallerClient)]
impl NamedInterfaceCaller {
    pub fn call_add_one(target: &ContractAddress, x: u32) -> u32 {
        named_interface_callee::ExplicitNamedInterfaceClient::new(target)
            .add_one(x)
    }
}
