//! An implementation of ERC20

use crate::interface::*;
use sdk::storage::{Map, NNInt, Value};
use sdk::*;

type Balance = NNInt;
type Allowance = NNInt;

#[sdk::storage]
#[derive(Debug)]
pub struct TokenContract {
    config: Value<Config>,
    balances: Map<ContractAddress, Balance>,
    allowances: Map<ContractAddress, Map<ContractAddress, Allowance>>,
    total_supply: NNInt,
}

#[contract_type]
#[derive(Debug)]
pub struct Config {
    pub administrator: ContractAddress,
    pub name: Option<Name>,
    pub symbol: Option<Symbol>,
    pub decimals: Option<u32>,
}

impl TokenContract {
    fn _init(&mut self, config: &Config) {
        if self.config.try_get().is_some() {
            throw(ERC20Error::AlreadyInitialized);
        }

        self.config.set(config)
    }

    fn _administrator(&self) -> ContractAddress {
        self.config.get().administrator
    }

    // These internal functions use the underlying storage to create an
    // abstraction whereby all balances and allowances exist, defaulting to
    // zero if never set. They do _not_ check authorization.

    // Balance stored at given owner address, or zero if empty
    fn _balance_amount_of(&self, owner: &ContractAddress) -> Amount {
        self.balances.at(owner).try_get().unwrap_or(0) as Amount
    }

    // Adjust balance at given owner address, from zero if empty
    fn _adjust_balance(&mut self, owner: &ContractAddress, diff: Difference) {
        // For never-stored-to addresses, this acts as
        // if the value was zero.  If the resulting balance
        // would be negative, this aborts.
        self.balances.at_mut(owner).strict_inc(diff);
    }

    fn _allowance(
        &self,
        owner: &ContractAddress,
        spender: &ContractAddress,
    ) -> Amount {
        self.allowances.at(owner).at(spender).try_get().unwrap_or(0) as Amount
    }

    // Adjust allowance for owner and spender, from zero if empty.
    // Returns resulting allowance.
    fn _adjust_allowance(
        &mut self,
        owner: &ContractAddress,
        spender: &ContractAddress,
        diff: Difference,
    ) {
        // This fails if the result would be negative
        self.allowances
            .at_mut(owner)
            .at_mut(spender)
            .strict_inc(diff);

        log_approval_event(owner, spender, diff);
    }

    fn _adjust_total_supply(&mut self, diff: Difference) {
        self.total_supply.strict_inc(diff);
    }

    fn _total_supply(&self) -> Amount {
        self.total_supply.try_get().unwrap_or(0) as Amount
    }
}

// This implements the ERC20 interface.
//
// The `contract` macro provides an exported entrypoint for each method.
//
#[contract(version = "0.1.0")]
impl ERC20 for TokenContract {
    fn initialize(&mut self, administrator: &ContractAddress) {
        self._init(&Config {
            administrator: *administrator,
            name: None,
            symbol: None,
            decimals: None,
        });
    }

    fn transfer(&mut self, receiver: &ContractAddress, amount: Amount) {
        let owner = get_caller();

        let diff = amount_to_diff(amount);
        self._adjust_balance(&owner, -diff);
        self._adjust_balance(receiver, diff);

        log_transfer_event(&owner, receiver, amount);
    }

    fn transfer_from(
        &mut self,
        owner: &ContractAddress,
        receiver: &ContractAddress,
        amount: Amount,
    ) {
        let spender = get_caller();

        let diff = amount_to_diff(amount);
        self._adjust_allowance(owner, &spender, -diff);
        self._adjust_balance(owner, -diff);
        self._adjust_balance(receiver, diff);

        log_transfer_event(owner, receiver, amount);
    }

    fn adjust_allowance(
        &mut self,
        spender: &ContractAddress,
        diff: Difference,
    ) {
        let owner = get_caller();

        self._adjust_allowance(&owner, spender, diff);
    }

    fn allowance(
        &self,
        owner: &ContractAddress,
        spender: &ContractAddress,
    ) -> Amount {
        self._allowance(owner, spender)
    }

    fn get_balance_of(&self, owner: &ContractAddress) -> Amount {
        self._balance_amount_of(owner)
    }

    fn get_balance(&self) -> Amount {
        let owner = get_caller();
        self._balance_amount_of(&owner)
    }

    fn mint(&mut self, receiver: &ContractAddress, amount: Amount) {
        let caller = get_caller();
        ensure(
            caller == self._administrator(),
            ERC20Error::MustBeContractOwner,
        );

        let diff = amount_to_diff(amount);
        self._adjust_balance(receiver, diff);
        self._adjust_total_supply(diff);

        log_transfer_event(CONTRACT_ADDR_ZERO, receiver, amount);
    }

    fn burn(&mut self, owner: &ContractAddress, amount: Amount) {
        let caller = get_caller();
        ensure(
            caller == self._administrator(),
            ERC20Error::MustBeContractOwner,
        );

        let diff = amount_to_diff(amount);
        self._adjust_balance(owner, -diff);
        self._adjust_total_supply(-diff);

        log_transfer_event(owner, CONTRACT_ADDR_ZERO, amount);
    }

    fn total_supply(&self) -> Amount {
        self._total_supply()
    }
}

#[contract(version = "0.1.0")]
impl ERC20Metadata for TokenContract {
    fn initialize(
        &mut self,
        administrator: &ContractAddress,
        name: Option<Name>,
        symbol: Option<Symbol>,
        decimals: Option<u32>,
    ) {
        self._init(&Config {
            administrator: *administrator,
            name,
            symbol,
            decimals,
        });
    }

    fn name(&self) -> Option<Name> {
        self.config.get().name
    }

    fn symbol(&self) -> Option<Symbol> {
        self.config.get().symbol
    }

    fn decimals(&self) -> Option<u32> {
        self.config.get().decimals
    }
}
