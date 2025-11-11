#![allow(unused)]

use groundhog_erc20_if::*;
use hex;
use sdk::*;
use thiserror::Error;
use std::collections::HashMap;

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

#[derive(Default, Clone)]
pub struct Model {
    pub contract_owner: Option<&'static ContractAddress>,
    pub total_supply: u128,
    pub other_bal: u128,
    pub balances: HashMap<&'static ContractAddress, Amount>,
    pub allowances: HashMap<(&'static ContractAddress, &'static ContractAddress), Amount>,
    pub events: Vec<ERC20Event<'static>>,
}

impl Model {
    pub fn initialize(&mut self, owner: &'static ContractAddress) {
        assert_eq!(self.contract_owner, None, "Model contract has already been initialized");

        self.contract_owner = Some(owner);
    }

    fn get_caller(&self) -> &'static ContractAddress {
        // In testing, the caller is always the contract owner
        self.contract_owner.expect("Model contract has not yet been initialized")
    }

    pub fn mint(&mut self, receiver: &'static ContractAddress, amount: Amount) {
        self.total_supply = self.total_supply.checked_add(amount).unwrap();

        let balance = self.balances.entry(receiver).or_insert(0);

        *balance = (*balance).checked_add(amount).unwrap();

        self.log_transfer_event(&CONTRACT_ADDR_ZERO, receiver, 100);
    }

    pub fn burn(&mut self, owner: &'static ContractAddress, amount: Amount) {
        self.total_supply = self.total_supply.checked_sub(amount).unwrap();

        let balance = self.balances.entry(owner).or_insert(0);

        *balance = (*balance).checked_sub(amount).unwrap();

        self.log_transfer_event(owner, &CONTRACT_ADDR_ZERO, amount);
    }

    pub fn adjust_allowance(&mut self,
        spender: &'static ContractAddress,
        diff: Difference,
    ) {
        let owner = self.get_caller();
        let entry = self.allowances.entry((owner, spender)).or_insert(0);

        *entry = (*entry).checked_add_signed(diff).unwrap();

        self.log_approval_event(owner, spender, diff);
    }

    pub fn transfer(&mut self,
        receiver: &'static ContractAddress,
        amount: Amount,
    ) {
        let owner = self.get_caller();
        let diff = amount as Difference; // XXX

        self.adjust_balance(owner, -diff);
        self.adjust_balance(receiver, diff);

        self.log_transfer_event(owner, receiver, amount);
    }

    fn adjust_balance(&mut self, owner: &'static ContractAddress, diff: Difference) {
        let bal = self.balances.entry(owner).or_insert(0);

        *bal = (*bal).checked_add_signed(diff).unwrap();
    }

    fn log_transfer_event(
        &mut self,
        from: &'static ContractAddress,
        to: &'static ContractAddress,
        amount: u128,
    ) {
        let event = TransferEvent::new(from, to, amount);

        self.events.push(ERC20Event::Transfer(event));
    }

    fn log_approval_event(
        &mut self,
        owner: &'static ContractAddress,
        spender: &'static ContractAddress,
        amount: i128,
    ) {
        let event = ApprovalEvent::new(owner, spender, amount);

        self.events.push(ERC20Event::Approval(event));
    }

    pub fn check_total_supply(&self, client: &mut ERC20Client) {
        assert_eq!(client.total_supply().unwrap(), self.total_supply);
    }

    pub fn check_owner_balance(&self, client: &mut ERC20Client) {
        let owner = self.get_caller();
        let model_bal = self.balances.get(owner).unwrap_or(&0);
        let client_bal = client.get_balance().unwrap();

        assert_eq!(&client_bal, model_bal);
    }

    pub fn check_other_balance(&self, client: &mut ERC20Client) {
        let model_bal = self.balances.get(&OTHER).unwrap_or(&0);
        let client_bal = client.get_balance_of(&OTHER).unwrap();

        assert_eq!(&client_bal, model_bal);
    }

    pub fn check_allowances(&self, client: &mut ERC20Client) {
        // TODO!
    }

    #[allow(unused)]
    pub fn check(&self, client: &mut ERC20Client) {
        self.check_total_supply(client);
        self.check_owner_balance(client);
        self.check_other_balance(client);
        self.check_allowances(client);
    }
}

use std::sync::Mutex;
static MODEL: Mutex<Option<Model>> = Mutex::new(None);

pub fn erc20_test(
    target_contract_hash: &sdk::types::SHA256Hash,
) -> ERC20TestResult<()> {

    let mut m = Model::default();

    let nonce: SHA256Hash = [7; 32].into();
    let deployed_addr = deploy_contract(&nonce, target_contract_hash)?;

    let mut client = ERC20Client::new(&deployed_addr);
    let client = &mut client;

    let owner = get_current_address()?;
    assert_eq!(owner, OWNER);

    let owner = &OWNER;
    let other = &OTHER;

    client.initialize(owner)?;
    m.initialize(owner);
        // Total supply and owner's balance are zero
        m.check_total_supply(client);
        m.check_owner_balance(client);

    // Mint
    let amt = 100;
    client.mint(owner, amt)?;
    m.mint(owner, amt);
        m.check_total_supply(client);
        m.check_owner_balance(client);

    // Burn
    let amt = 50;
    client.burn(owner, amt)?;
    m.burn(owner, amt);
        m.check_total_supply(client);
        m.check_owner_balance(client);

    // Burn too much
    assert!(m.balances[owner] < 60);
    assert!(matches!(
        client.burn(owner, 60),
        Err(ClientError::Aborted(_ /* ERC20Error::TooLarge */))
    ));

    // Transfer
    let amt = 25;
    client.transfer(other, amt).unwrap();
    m.transfer(other, amt);
        m.check_owner_balance(client);
        m.check_other_balance(client);

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
        m.check_owner_balance(client);
        m.check_total_supply(client);

    // Approve spending by "other"
    client.adjust_allowance(other, 17);
    m.adjust_allowance(other, 17);

    // Check everything against the model
    m.check(client);

    // Store the model in a global, for retrieval in the function below
    *MODEL.lock().unwrap() = Some(m);

    Ok(())
}

use sdk::types::event::TxEmittedEvent;

pub fn erc20_test_event_log_check(events: &[TxEmittedEvent]) {

    events.iter().for_each(print_event);

    // Retrieve the last-constructed model from a global
    let model = MODEL.lock().unwrap().clone().unwrap();

    // assert_eq!(events.len(), model.events.len());

    let mut events = events.iter();

    // For each event from the modelled event log ...
    for (i, mevent) in model.events.iter().enumerate() {
        // Get the next event from the test event log
        let e = events.next().expect("Unexpected end of event log");
        let sig = event_topic(e, 0).expect("no event signature");

        match mevent {
            ERC20Event::Transfer(me) => {
                let te = to_transfer_event(e);
                assert_eq!(&te, me, "Event {} doesn't match", i);
            }
            ERC20Event::Approval(me) => {
                let ae = to_approval_event(e);
                assert_eq!(&ae, me, "Event {} doesn't match", i);
            }
        }
    }

    if events.len() > 0 {
        panic!("Test contract produced {} more events than expected", events.len());
    }
}

fn to_transfer_event(e: &TxEmittedEvent) -> TransferEvent<'_> {
    let sig = event_topic(e, 0).expect("no event signature");
    assert_eq!(sig, TRANSFER_EVENT_SIG, "Unexpected event signature");

    let from: &ContractAddress = event_topic(e, 1).expect("Missing topic").into();
    let to: &ContractAddress = event_topic(e, 2).expect("Missing topic").into();

    let payload = event_payload(e).expect("malformed event");
    let amount: u128 = sdk::internal::decode_xdr(payload).expect("couldn't parse event payload");

    TransferEvent::new(from, to, amount)
}

fn to_approval_event(e: &TxEmittedEvent) -> ApprovalEvent<'_> {
    let sig = event_topic(e, 0).expect("no event signature");
    assert_eq!(sig, APPROVAL_EVENT_SIG, "Unexpected event signature");

    let owner: &ContractAddress = event_topic(e, 1).expect("Missing topic").into();
    let spender: &ContractAddress = event_topic(e, 2).expect("Missing topic").into();

    let payload = event_payload(e).expect("malformed event");
    let amount: i128 = sdk::internal::decode_xdr(payload).expect("couldn't parse event payload");

    ApprovalEvent::new(owner, spender, amount)
}

fn print_event(e: &TxEmittedEvent) {
    let sig = event_topic(e, 0).expect("no event signature");

    match sig {
        TRANSFER_EVENT_SIG => {
            let from: &ContractAddress =
                event_topic(e, 1).expect("no from address").into();
            let to: &ContractAddress =
                event_topic(e, 2).expect("no to address").into();
            let payload = event_payload(e).expect("malformed event");
            let amount: u128 = sdk::internal::decode_xdr(payload).unwrap();

            if from == CONTRACT_ADDR_ZERO {
                println!(
                    "MINT_TO {} amount:{:?}",
                    hex::encode(to),
                    amount
                );
            } else if to == CONTRACT_ADDR_ZERO {
                println!(
                    "BURN_FROM {} amount:{:?}",
                    hex::encode(from),
                    amount
                );
            } else {
                println!(
                    "TRANSFER {} {} amount:{:?}",
                    hex::encode(from),
                    hex::encode(to),
                    amount,
                );
            }
        }
        APPROVAL_EVENT_SIG => {
            let payload = event_payload(e).expect("malformed event");
            let amount: i128 = sdk::internal::decode_xdr(payload).unwrap();
            println!("APPROVE {:?}", amount);
        }
        _ => {
            panic!("Unexpected topic signature");
        }
    }
}
