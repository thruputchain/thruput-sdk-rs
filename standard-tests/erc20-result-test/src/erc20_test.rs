use groundhog_erc20_if::*;
use sdk::*;
use thiserror::Error;
use crate::model::Model;
use std::sync::Mutex;
use sdk::types::event::TxEmittedEvent;

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

pub const NONCE: SHA256Hash = SHA256Hash::new([7; 32]);
pub const OWNER: ContractAddress = ContractAddress::new([0xFF; 32]);
    const OTHER: ContractAddress = ContractAddress::new([0xBB; 32]);

// It's currently difficult to extract state from mock contracts, so we use this global to keep
// track of state during the test run. (This won't work if the below functions are not run one
// after the other, or if concurrent tests are run in different threads.)
static MODEL: Mutex<Option<Model>> = Mutex::new(None);

/// Test the logic of an ERC20 contract
pub fn erc20_test(
    target_contract_hash: &sdk::types::SHA256Hash,
) -> ERC20TestResult<()> {

    let deployed_addr = deploy_contract(&NONCE, target_contract_hash)?;
    let mut client = ERC20Client::new(&deployed_addr);
    let client = &mut client;

    let owner = get_current_address()?;
    assert_eq!(owner, OWNER);

    let owner = &OWNER;
    let other = &OTHER;

    let mut m = Model::default();

    client.initialize(owner)?;
    m.initialize(owner);
        // Total supply and owner's balance are zero
        m.check_total_supply(client);
        m.check_balances(client);

    // Mint
    let amt = 100;
    client.mint(owner, amt)?;
    m.mint(owner, amt);
        m.check_total_supply(client);
        m.check_balances(client);

    // Burn
    let amt = 50;
    client.burn(owner, amt)?;
    m.burn(owner, amt);
        m.check_total_supply(client);
        m.check_balances(client);

    // Burn too much
    let amt = 60;
    assert!(m.balances[owner] < amt);
    assert!(matches!(
        client.burn(owner, amt),
        Err(ClientError::Aborted(_ /* ERC20Error::TooLarge */))
    ));

    // Transfer
    let amt = 25;
    client.transfer(other, amt).unwrap();
    m.transfer(other, amt);
        m.check_balances(client);

    // Transfer too much
    assert!(m.balances[owner] < 1000);
    assert!(matches!(
        client.transfer(other, 1000),
        Err(ClientError::Aborted(_ /* ERC20Error::TooLarge */))
    ));

    // Burn from other address
    let amt = 5;
    client.burn(other, amt).unwrap();
    m.burn(other, amt);
        m.check_total_supply(client);
        m.check_balances(client);

    // Approve spending by "other"
    let amt = 17;
    client.adjust_allowance(other, amt)?;
    m.adjust_allowance(other, amt);

    // Withdraw too much approval
    assert!(matches!(
        client.adjust_allowance(other, -20),
        Err(ClientError::Aborted(_)),
    ));

    // Withdraw some approval
    let amt = 10;
    client.adjust_allowance(other, -amt)?;
    m.adjust_allowance(other, -amt);

    // Check everything against the model
    m.check(client);

    // Store the model in a global, for retrieval in the function below
    *MODEL.lock().unwrap() = Some(m);

    Ok(())
}

/// Check that the given event log matches what the model predicted
pub fn erc20_test_event_log_check(events: &[TxEmittedEvent]) {

    // Retrieve the last-constructed model from a global
    let model = MODEL.lock().unwrap().clone().unwrap();

    model.check_events(events);
}
