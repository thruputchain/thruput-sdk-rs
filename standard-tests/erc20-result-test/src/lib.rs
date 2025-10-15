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

pub fn erc20_test(
    target_contract_hash: &sdk::types::SHA256Hash,
) -> ERC20TestResult<()> {
    let nonce: SHA256Hash = [7; 32].into();
    let deployed_addr = deploy_contract(&nonce, target_contract_hash)?;

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
