//! An implementation of ERC20

use crate::interface::*;
use sdk::storage::{Balance, Map, Value};
use sdk::*;

type Allowance = Balance;

#[sdk::storage]
#[cfg_attr(feature = "dev", derive(Debug))]
pub struct TokenContract {
    config: Value<Config>,
    balances: Map<ContractAddress, Balance>,
    allowances: Map<ContractAddress, Map<ContractAddress, Allowance>>,
    total_supply: Balance,
}

#[cfg(feature = "metadata")]
#[contract_type]
#[cfg_attr(feature = "dev", derive(Debug))]
pub struct Config {
    pub administrator: ContractAddress,
    pub name: Option<Name>,
    pub symbol: Option<Symbol>,
    pub decimals: Option<u32>,
}

#[cfg(not(feature = "metadata"))]
#[contract_type]
#[cfg_attr(feature = "dev", derive(Debug))]
pub struct Config {
    pub administrator: ContractAddress,
}

impl TokenContract {
    fn _init(&mut self, config: &Config) {
        if self.config.try_get().none_if_missing().is_some() {
            throw(ERC20Error::AlreadyInitialized);
        }

        self.config.set(config)
    }

    fn _administrator(&self) -> ContractAddress {
        self.config.get().administrator
    }

    // These internal functions do _not_ check authorization.

    fn _balance_amount_of(&self, owner: &ContractAddress) -> Amount {
        self.balances.at(owner).get()
    }

    // A balance credit is lazy: the total-supply invariant proves that a
    // valid transfer or mint cannot overflow an individual balance.
    fn _credit_balance(&mut self, owner: &ContractAddress, amount: Amount) {
        self.balances
            .at_mut(owner)
            .try_increase(amount)
            .unwrap_or_else(|_| throw(ERC20Error::TooLarge));
    }

    fn _debit_balance(&mut self, owner: &ContractAddress, amount: Amount) {
        self.balances
            .at_mut(owner)
            .try_decrease(amount)
            .unwrap_or_else(|_| throw(ERC20Error::TooLarge));
    }

    fn _allowance(
        &self,
        owner: &ContractAddress,
        spender: &ContractAddress,
    ) -> Amount {
        self.allowances.at(owner).at(spender).get()
    }

    // Adjust allowance for owner and spender, from zero if empty.
    // Returns resulting allowance.
    fn _adjust_allowance(
        &mut self,
        owner: &ContractAddress,
        spender: &ContractAddress,
        diff: Difference,
    ) {
        let mut owner_allowances = self.allowances.at_mut(owner);
        let mut allowance = owner_allowances.at_mut(spender);
        let result = if diff >= 0 {
            // Allowances are not bounded by total supply, so credit them
            // strictly to preserve the interface's immediate TooLarge error.
            allowance.try_checked_increase(diff as u128)
        } else {
            allowance.try_decrease(diff.unsigned_abs())
        };
        result.unwrap_or_else(|_| throw(ERC20Error::TooLarge));

        log_approval_event(owner, spender, diff);
    }

    fn _checked_credit_total_supply(&mut self, amount: Amount) {
        self.total_supply
            .try_checked_increase(amount)
            .unwrap_or_else(|_| throw(ERC20Error::TooLarge));
    }

    fn _debit_total_supply(&mut self, amount: Amount) {
        self.total_supply
            .try_decrease(amount)
            .unwrap_or_else(|_| throw(ERC20Error::TooLarge));
    }

    fn _total_supply(&self) -> Amount {
        self.total_supply.get()
    }
}

// This implements the ERC20 interface.
//
// The `contract` macro provides an exported entrypoint for each method.
//
#[contract(version = "0.1.2")]
impl ERC20 for TokenContract {
    fn initialize(&mut self, administrator: &ContractAddress) {
        self._init(&Config {
            administrator: *administrator,

            #[cfg(feature = "metadata")]
            name: None,

            #[cfg(feature = "metadata")]
            symbol: None,

            #[cfg(feature = "metadata")]
            decimals: None,
        });
    }

    fn transfer(&mut self, receiver: &ContractAddress, amount: Amount) {
        let owner = get_caller();

        self._debit_balance(&owner, amount);
        self._credit_balance(receiver, amount);

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
        self._debit_balance(owner, amount);
        self._credit_balance(receiver, amount);

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

        // Establish the checked supply bound first. Once it holds, the lazy
        // receiver credit cannot overflow an individual balance.
        self._checked_credit_total_supply(amount);
        self._credit_balance(receiver, amount);

        log_transfer_event(CONTRACT_ADDR_ZERO, receiver, amount);
    }

    fn burn(&mut self, owner: &ContractAddress, amount: Amount) {
        let caller = get_caller();
        ensure(
            caller == self._administrator(),
            ERC20Error::MustBeContractOwner,
        );

        self._debit_balance(owner, amount);
        self._debit_total_supply(amount);

        log_transfer_event(owner, CONTRACT_ADDR_ZERO, amount);
    }

    fn total_supply(&self) -> Amount {
        self._total_supply()
    }
}

#[cfg(feature = "metadata")]
#[contract(version = "0.1.2")]
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
