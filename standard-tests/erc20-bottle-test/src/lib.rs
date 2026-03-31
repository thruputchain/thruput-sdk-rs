#![no_std]

pub use bottle_test_if::*;
use erc20_test::*;
use sdk::storage::Value;
use sdk::*;

#[derive(Debug)]
#[storage]
pub struct BT {
    target_addr: Value<Option<ContractAddress>>,
}

#[contract(version = "0.1.0")]
impl BottleTest for BT {
    fn setup(&mut self, target_contract_hash: &SHA256Hash) {
        let target_addr = erc20_test_setup(target_contract_hash);

        self.target_addr.set(&Some(target_addr));
    }

    fn run(&mut self) {
        let target_addr = self.target_addr.get().unwrap_or_else(||
            panic!("erc20-bottle-test: target_addr unset; please run setup() before run()"));

        let _ = erc20_test(&target_addr);
    }
}
