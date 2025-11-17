#![no_std]

pub use bottle_test_if::*;
use erc20_test::*;
use sdk::*;

#[derive(Debug)]
#[storage]
pub struct BT {
    target_addr: Option<ContractAddress>,
}

#[contract]
impl BottleTest for BT {
    #[init]
    fn setup(&mut self, target_contract_hash: &SHA256Hash) {
        let target_addr = erc20_test_setup(target_contract_hash).unwrap();

        self.target_addr = Some(target_addr);
    }

    fn run(&mut self) {
        let target_addr = self.target_addr.as_ref().unwrap_or_else(||
            panic!("erc20-bottle-test: target_addr unset; please run setup() before run()"));

        erc20_test(target_addr).unwrap_or_else(|e|
            panic!("erc20-test failed: {}", e));
    }
}
