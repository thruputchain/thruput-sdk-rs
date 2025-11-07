#![allow(unused)]

use groundhog_erc20_if::{ERC20Client, ERC20Error, ERC20Event, TransferEvent};
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
    pub events: Vec<ERC20Event<'static>>,
}

impl Model {
    pub fn log_transfer_event(&mut self,
        from: &'static ContractAddress,
        to: &'static ContractAddress,
        amount: u128,
    ) {
        let event = TransferEvent::new(from, to, amount);

        self.events.push(ERC20Event::Transfer(event));
    }

    pub fn check_total_supply(&self, client: &mut ERC20Client) {
        assert_eq!(client.total_supply().unwrap(), self.total_supply);
    }

    pub fn check_owner_balance(&self, client: &mut ERC20Client) {
        assert_eq!(client.get_balance().unwrap(), self.owner_bal);
    }

    pub fn check_other_balance(&self, client: &mut ERC20Client) {
        assert_eq!(client.get_balance_of(&OTHER).unwrap(), self.other_bal);
    }

    #[allow(unused)]
    pub fn check(&self, client: &mut ERC20Client) {
        self.check_total_supply(client);
        self.check_owner_balance(client);
        self.check_other_balance(client);
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
    assert!(m.owner_bal < 60);
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

    // Transfer too much
    assert!(m.owner_bal < 1000);
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

    Ok(())
}

pub fn erc20_test_event_log_check(events: &[sdk::types::event::TxEmittedEvent]) {
    // TODO: check that the event log looks like it should

    assert_eq!(events.len(), 1);
    let event = &events[0];
    let payload = event_payload(event).expect("malformed event");
    let amount: i128 = sdk::internal::decode_xdr(payload).unwrap();

    println!("{:?}", amount);
}
