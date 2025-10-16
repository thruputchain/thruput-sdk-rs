#![no_std]

pub use bottle_test_if::*;
use groundhog_erc20_if::ERC20Client;
use sdk::*;

#[derive(Copy, Clone, Default)]
pub struct BT;

#[contract(storage = "false")]
impl BottleTest for BT {
    fn run(&mut self, target_contract_hash: &SHA256Hash) {
        // Deploy an ERC20 contract
        let nonce: SHA256Hash = [7; 32].into();
        let deployed_addr =
            deploy_contract(&nonce, target_contract_hash).unwrap();
        let mut client = ERC20Client::new(&deployed_addr);

        let owner = get_current_address().unwrap();
        client.initialize(&owner).unwrap();

        assert_eq!(client.total_supply().unwrap(), 0);
        assert_eq!(client.get_balance().unwrap(), 0);

        client.mint(&owner, 100).unwrap();
        assert_eq!(client.get_balance().unwrap(), 100);

        let other: ContractAddress = [0xBB; 32].into();
        client.transfer(&other, 25).unwrap();
        assert_eq!(client.get_balance_of(&other).unwrap(), 25);
        assert_eq!(client.get_balance_of(&owner).unwrap(), 75);

        client.burn(&other, 5).unwrap();
        assert_eq!(client.get_balance_of(&other).unwrap(), 20);

        assert_eq!(client.total_supply().unwrap(), 95);
    }
}
