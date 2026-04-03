use sdk::*;

#[contract(interface = "TraitOnly")]
pub trait TraitOnly {
    fn ping() -> u32;
}

fn main() {
    let _client = TraitOnlyClient::new(&ContractAddress::default());
    let _entrypoint: ContractEntrypointID = TraitOnlyEntrypoints::ping.into();
}
