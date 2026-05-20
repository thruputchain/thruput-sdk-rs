#![allow(unexpected_cfgs)]

use sdk::*;

pub struct MyContract;

#[contract(interface = "RenamedInterface")]
pub trait MyTrait {
    fn do_thing() -> u32;
}

#[contract(interface = "RenamedInterface")]
impl MyTrait for MyContract {
    fn do_thing() -> u32 {
        42
    }
}

fn main() {
    let _client = RenamedInterfaceClient::new(&ContractAddress::default());
    let _entrypoint: ContractEntrypointID =
        RenamedInterfaceEntrypoints::do_thing.into();
}
