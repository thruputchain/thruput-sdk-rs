use crate::model::Model;
use groundhog_erc20_if::*;
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

    #[error("erc20-test error: {0}")]
    Custom(String),
}

pub type ERC20TestResult<T> = core::result::Result<T, ERC20TestError>;

pub const NONCE: SHA256Hash = SHA256Hash::new([7; 32]);
pub const OWNER: ContractAddress = ContractAddress::new([0xFF; 32]);
const OTHER: ContractAddress = ContractAddress::new([0xBB; 32]);

pub fn erc20_test_setup(
    target_contract_hash: &SHA256Hash,
) -> ERC20TestResult<ContractAddress> {
    // Deploy the target contract
    let deployed_addr = deploy_contract(&NONCE, target_contract_hash);

    Ok(deployed_addr)
}

/// Test the logic of an ERC20 contract
pub fn erc20_test(deployed_addr: &ContractAddress) -> ERC20TestResult<Model> {
    let owner = get_current_address();
    let other = &OTHER;

    let mut m = Model::default();

    let mut client = ERC20Client::new(&deployed_addr);
    let client = &mut client;

    client.initialize(&owner);
    m.initialize(&owner);

    // Mint
    let amt = 100;
    client.mint(&owner, amt);
    m.mint(&owner, amt);

    // Burn
    let amt = 50;
    client.burn(&owner, amt);
    m.burn(&owner, amt);

    #[cfg(not(feature = "no_check_aborts"))]
    {
        // Burn too much
        let amt = 60;
        assert!(m.balances[&owner] < amt);
        assert!(matches!(
            client.burn(&owner, amt),
            Err(ClientError::Aborted(_ /* ERC20Error::TooLarge */))
        ));
    }

    // Transfer
    let amt = 25;
    client.transfer(other, amt);
    m.transfer(other, amt);

    #[cfg(not(feature = "no_check_aborts"))]
    {
        // Transfer too much
        assert!(m.balances[&owner] < 1000);
        assert!(matches!(
            client.transfer(other, 1000),
            Err(ClientError::Aborted(_ /* ERC20Error::TooLarge */))
        ));
    }

    // Burn from other address
    let amt = 5;
    client.burn(other, amt);
    m.burn(other, amt);

    // Approve spending by "other"
    let amt = 17;
    client.adjust_allowance(other, amt);
    m.adjust_allowance(other, amt);

    #[cfg(not(feature = "no_check_aborts"))]
    // Withdraw too much approval
    assert!(matches!(
        client.adjust_allowance(other, -20),
        Err(ClientError::Aborted(_)),
    ));

    // Withdraw some approval
    let amt = 10;
    client.adjust_allowance(other, -amt);
    m.adjust_allowance(other, -amt);

    // Check client state against the model
    m.check(client);

    Ok(m)
}
