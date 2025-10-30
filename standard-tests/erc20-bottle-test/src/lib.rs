#![no_std]

pub use bottle_test_if::*;
use groundhog_erc20_if::ERC20Client;
use sdk::*;

#[derive(Debug)]
#[storage]
pub struct BT {
    target_addr: Option<ContractAddress>,
}

const OTHER: ContractAddress = ContractAddress::new([0xBB; 32]);

#[derive(Default)]
struct Model {
    pub total_supply: u128,
    pub owner_bal: u128,
    pub other_bal: u128,
}

impl Model {
    pub fn check_total_supply(&self, client: &mut ERC20Client) {
        assert_eq!(client.total_supply().unwrap(), self.total_supply);
    }
    pub fn check_owner_balance(&self, client: &mut ERC20Client) {
        assert_eq!(client.get_balance().unwrap(), self.owner_bal);
    }
    pub fn check_other_balance(&self, client: &mut ERC20Client) {
        assert_eq!(client.get_balance_of(&OTHER).unwrap(), self.other_bal);
    }
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
        // Keep our own model of contract state to compare against
        let mut m = Model::default();

        let target_addr = self.target_addr.as_ref().unwrap_or_else(||
            contract_panic!("erc20-bottle-test: target_addr unset; please run setup() before run()"));

        let owner = get_current_address().unwrap();
        let owner = &owner;
        let other = &OTHER;

        let mut client = ERC20Client::new(target_addr);
        let client = &mut client;
        client.initialize(owner).unwrap_or_else(|e| {
            contract_panic!("erc20-bottle-test: initialize() failed: {}", e)
        });

        // Total supply and owner's balance are zero
        m.check_total_supply(client);
        m.check_owner_balance(client);

        // Mint
        let amt = 100;
        client.mint(&owner, 100).unwrap();
        m.total_supply += amt;
        m.owner_bal += amt;
        m.check_total_supply(client);
        m.check_owner_balance(client);

        // Burn
        let amt = 50;
        client.burn(&owner, amt).unwrap();
        m.total_supply -= amt;
        m.owner_bal -= amt;
        m.check_total_supply(client);
        m.check_owner_balance(client);

        // Burn too much (only caught immediately if the erc20 contract uses "checked" increment)
        #[cfg(feature = "fail-fast")]
        assert!(matches!(
            client.burn(owner, 60),
            Err(ClientError::Aborted(_ /* ERC20Error::TooLarge */))
        ));

        // Transfer
        let amt = 25;
        client.transfer(other, amt).unwrap();
        m.owner_bal -= amt;
        m.other_bal += amt;
        m.check_owner_balance(client);
        m.check_other_balance(client);

        // Transfer too much (only caught immediately if the erc20 contract uses "checked" increment)
        #[cfg(feature = "fail-fast")]
        assert!(matches!(
            client.transfer(other, 1000),
            Err(ClientError::Aborted(_ /* ERC20Error::TooLarge */))
        ));

        // Burn from other address
        client.burn(&other, 5).unwrap();
        m.total_supply -= 5;
        m.other_bal -= 5;
        m.check_owner_balance(client);
        m.check_total_supply(client);

        // TODO: test allowances
    }
}
