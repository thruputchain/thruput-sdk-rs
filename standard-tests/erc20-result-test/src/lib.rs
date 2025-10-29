use groundhog_erc20_if::{ERC20Client, ERC20Error};
use sdk::*;
use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum ERC20TestError {
    #[error("contract error: {0}")]
    Contract(#[from] ERC20Error),

    #[error("client error: {0}")]
    Client(#[from] ClientError<ERC20Error>),

    #[error("system error: {0}")]
    Sys(#[from] SysErr),
}

pub type ERC20TestResult<T> = core::result::Result<T, ERC20TestError>;

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

pub fn erc20_test(
    target_contract_hash: &sdk::types::SHA256Hash,
) -> ERC20TestResult<()> {

    // Keep our own model of contract state to compare against
    let mut m = Model::default();

    let nonce: SHA256Hash = [7; 32].into();
    let deployed_addr = deploy_contract(&nonce, target_contract_hash)?;

    let mut client = ERC20Client::new(&deployed_addr);
    let client = &mut client;

    let owner = get_current_address()?;
    let owner = &owner;
    let other = &OTHER;

    client.initialize(&owner)?;

    // Total supply and owner's balance are zero
    m.check_total_supply(client);
    m.check_owner_balance(client);

    // Mint
    let amt = 100;
    client.mint(&owner, 100)?;
    m.total_supply += amt;
    m.owner_bal += amt;
    m.check_total_supply(client);
    m.check_owner_balance(client);

    // Burn
    let amt = 50;
    client.burn(&owner, amt)?;
    m.total_supply -= amt;
    m.owner_bal -= amt;
    m.check_total_supply(client);
    m.check_owner_balance(client);

    // Burn too much
    assert_eq!(client.burn(owner, 60), Err(ClientError::Aborted(ERC20Error::TooLarge)));

    // Transfer
    let amt = 25;
    client.transfer(other, amt).unwrap();
    m.owner_bal -= amt;
    m.other_bal += amt;
    m.check_owner_balance(client);
    m.check_other_balance(client);

    // Transfer too much
    assert_eq!(client.transfer(other, 1000), Err(ClientError::Aborted(ERC20Error::TooLarge)));

    // Burn from other address
    client.burn(&other, 5).unwrap();
    m.total_supply -= 5;
    m.other_bal -= 5;
    m.check_owner_balance(client);
    m.check_total_supply(client);

    Ok(())
}
