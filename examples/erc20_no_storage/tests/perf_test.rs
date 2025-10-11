use core::error::Error;
use sdk::contract_test::*;
use sdk::*;

use erc20_no_storage::ERC20Client;

// working directory for tests is the (test) package root
const WASM_PATH: &str = "../../target/optimized/erc20_no_storage.wasm";

#[test]
fn run() {
    test_wasm(WASM_PATH, Test::default());
}

#[derive(Copy, Clone, Default)]
pub struct Test;

impl ContractTest for Test {
    fn run(&self, contract_hash: &SHA256Hash) -> Result<(), Box<dyn Error>> {
        let nonce: SHA256Hash = [7; 32].into();
        let deployed_addr = deploy_contract(&nonce, contract_hash)?;
        let mut client = ERC20Client::new(&deployed_addr);

        let owner = get_current_address()?;
        client.initialize(&owner)?;

        assert_eq!(client.total_supply()?, 0);
        assert_eq!(client.get_balance()?, 0);

        client.mint(&owner, 100)?;
        assert_eq!(client.get_balance()?, 100);

        let other: ContractAddress = [0xBB; 32].into();
        client.transfer(&other, 25)?;
        assert_eq!(client.get_balance_of(&other)?, 25);
        assert_eq!(client.get_balance_of(&owner)?, 75);

        client.burn(&other, 5)?;
        assert_eq!(client.get_balance_of(&other)?, 20);

        assert_eq!(client.total_supply()?, 95);

        Ok(())
    }
}
