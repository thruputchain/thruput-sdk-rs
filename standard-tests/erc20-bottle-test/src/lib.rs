#![no_std]

pub use bottle_test_if::*;
use groundhog_erc20_if::ERC20Client;
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
        // Deploy an ERC20 contract
        let nonce: SHA256Hash = [7; 32].into();
        let target_addr =
            deploy_contract(&nonce, target_contract_hash).unwrap();

        self.target_addr = Some(target_addr);
    }

    fn run(&mut self) {
        let target_addr = self.target_addr.as_ref().unwrap_or_else(||
            contract_panic!("erc20-bottle-test: target_addr unset; please run setup() before run()"));

        let mut client = ERC20Client::new(target_addr);

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
