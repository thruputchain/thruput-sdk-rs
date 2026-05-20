#![allow(unexpected_cfgs)]

use sdk::*;

#[derive(Default)]
pub struct BareContract;

#[contract(storage = false)]
impl BareContract {
    pub fn ping(&self) -> u32 {
        7
    }
}

fn main() {
    let _client = BareContractClient::new(&ContractAddress::default());
    let _entrypoint: ContractEntrypointID =
        BareContractEntrypoints::ping.into();
    let _entrypoint_u64 = BareContractEntrypoints::ping.into_u64();
}
