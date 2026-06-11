use alloc::collections::BTreeMap as Map;
use alloc::vec::Vec;
use groundhog_erc20_if::*;
use sdk::types::event::TxEmittedEvent;
use sdk::*;

/// A model of an ERC20 contract, for comparing state to ERC20 contracts under test
#[derive(Default, Clone)]
pub struct Model {
    pub contract_owner: Option<ContractAddress>,
    pub total_supply: u128,
    pub balances: Map<ContractAddress, Amount>,
    pub allowances: Map<(ContractAddress, ContractAddress), Amount>,
    pub events: Vec<StdERC20Event>,
}

// Implement the same interface that ERC20 does, and keep track of
// the expected state
impl Model {
    pub fn initialize(&mut self, owner: &ContractAddress) {
        assert_eq!(
            self.contract_owner, None,
            "Model contract has already been initialized"
        );

        self.contract_owner = Some(*owner);
    }

    fn get_caller(&self) -> ContractAddress {
        // In testing, the caller is always the contract owner
        self.contract_owner
            .expect("Model contract has not yet been initialized")
    }

    pub fn mint(&mut self, receiver: &ContractAddress, amount: Amount) {
        self.total_supply = self.total_supply.checked_add(amount).unwrap();

        let balance = self.balances.entry(*receiver).or_insert(0);

        *balance = (*balance).checked_add(amount).unwrap();

        self.log_transfer_event(CONTRACT_ADDR_ZERO, receiver, amount);
    }

    pub fn burn(&mut self, owner: &ContractAddress, amount: Amount) {
        self.total_supply = self.total_supply.checked_sub(amount).unwrap();

        let balance = self.balances.entry(*owner).or_insert(0);

        *balance = (*balance).checked_sub(amount).unwrap();

        self.log_transfer_event(owner, CONTRACT_ADDR_ZERO, amount);
    }

    pub fn adjust_allowance(
        &mut self,
        spender: &ContractAddress,
        diff: Difference,
    ) {
        let owner = self.get_caller();
        let entry = self.allowances.entry((owner, *spender)).or_insert(0);

        *entry = (*entry).checked_add_signed(diff).unwrap();

        self.log_approval_event(&owner, spender, diff);
    }

    pub fn transfer(
        &mut self,
        receiver: &'static ContractAddress,
        amount: Amount,
    ) {
        let owner = self.get_caller();
        let diff: Difference = amount.try_into().unwrap();

        self.adjust_balance(&owner, -diff);
        self.adjust_balance(receiver, diff);

        self.log_transfer_event(&owner, receiver, amount);
    }

    fn adjust_balance(&mut self, owner: &ContractAddress, diff: Difference) {
        let bal = self.balances.entry(*owner).or_insert(0);

        *bal = (*bal).checked_add_signed(diff).unwrap();
    }

    fn log_transfer_event(
        &mut self,
        from: &ContractAddress,
        to: &ContractAddress,
        amount: u128,
    ) {
        let event = ERC20Event::Transfer(TransferEvent::new(from, to, amount));

        self.events.push((&event).into());
    }

    fn log_approval_event(
        &mut self,
        owner: &ContractAddress,
        spender: &ContractAddress,
        amount: i128,
    ) {
        let event =
            ERC20Event::Approval(ApprovalEvent::new(owner, spender, amount));

        self.events.push((&event).into());
    }
}

// Check our state against that of a supplied ERC20 contract
impl Model {
    pub fn check_total_supply(&self, client: &ERC20Client) {
        assert_eq!(client.total_supply(), self.total_supply);
    }

    pub fn check_balances(&self, client: &ERC20Client) {
        for (addr, model_bal) in &self.balances {
            let client_bal = client.get_balance_of(addr);

            assert_eq!(&client_bal, model_bal);
        }
    }

    pub fn check_allowances(&self, client: &ERC20Client) {
        for ((owner, spender), model_allowance) in &self.allowances {
            let allowance = client.allowance(owner, spender);

            assert_eq!(&allowance, model_allowance);
        }
    }

    pub fn check(&self, client: &ERC20Client) {
        self.check_total_supply(client);
        self.check_balances(client);
        self.check_allowances(client);
    }
}

/// Compare an expected event log (from the model) against the events a
/// transaction actually emitted.
pub fn check_events(expected: &[StdERC20Event], events: &[TxEmittedEvent]) {
    let mut events = events.iter();

    // For each event from our modelled event log ...
    for (i, model_event) in expected.iter().enumerate() {
        // Get the next event from the transaction event log
        let emitted_event: EmittedEvent =
            events.next().expect("Unexpected end of event log").into();

        let event: ERC20Event = emitted_event.try_into().unwrap_or_else(|e| {
            panic!("Couldn't parse emitted event as an ERC20 event: {}", e)
        });

        let event: StdERC20Event = (&event).into();

        assert_eq!(&event, model_event, "Event {} doesn't match", i);
    }

    if events.len() > 0 {
        panic!(
            "Test contract produced {} more events than expected",
            events.len()
        );
    }
}

impl Model {
    /// Check model state against events emitted by a transaction
    pub fn check_events(&self, events: &[TxEmittedEvent]) {
        check_events(&self.events, events);
    }
}
