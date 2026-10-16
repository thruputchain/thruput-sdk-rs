//! An implementation of ERC20

use crate::interface::*;
use thruput_sdk::block::get_block_height;
use thruput_sdk::storage::{Balance, Deletable, Map, NNInt, Node, Value};
use thruput_sdk::*;

#[thruput_sdk::storage]
#[derive(thruput_sdk::Deletable)]
#[cfg_attr(feature = "dev", derive(Debug))]
pub struct Allowance {
    amount: NNInt,
    expiry: Value<BlockHeight>,
}

#[thruput_sdk::storage]
#[cfg_attr(feature = "dev", derive(Debug))]
pub struct TokenContract {
    config: Value<Config>,
    balances: Map<ContractAddress, Balance>,
    allowances: Map<ContractAddress, Map<ContractAddress, Node<Allowance>>>,
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

    fn _lazy_increase_balance(
        &mut self,
        owner: &ContractAddress,
        amount: Amount,
    ) {
        self.balances
            .at_mut(owner)
            .try_lazy_increase(amount)
            .unwrap_or_else(|_| throw(ERC20Error::TooLarge));
    }

    fn _decrease_balance(&mut self, owner: &ContractAddress, amount: Amount) {
        self.balances
            .at_mut(owner)
            .try_decrease(amount)
            .unwrap_or_else(|_| throw(ERC20Error::TooLarge));
    }

    fn _allowance_is_live(
        &self,
        owner: &ContractAddress,
        spender: &ContractAddress,
    ) -> bool {
        self.allowances
            .at(owner)
            .at(spender)
            .expiry
            .try_get()
            .none_if_missing()
            .is_none_or(|until| get_block_height() <= until)
    }

    // The stored amount, whether or not it has expired.
    fn _stored_allowance(
        &self,
        owner: &ContractAddress,
        spender: &ContractAddress,
    ) -> Amount {
        self.allowances
            .at(owner)
            .at(spender)
            .amount
            .try_get()
            .none_if_missing()
            .unwrap_or(0) as Amount
    }

    fn _allowance(
        &self,
        owner: &ContractAddress,
        spender: &ContractAddress,
    ) -> Amount {
        if !self._allowance_is_live(owner, spender) {
            return 0;
        }
        self._stored_allowance(owner, spender)
    }

    // Delete the stored allowance under an exclusive lock, returning the
    // amount removed.
    fn _remove_allowance(
        &mut self,
        owner: &ContractAddress,
        spender: &ContractAddress,
    ) -> Difference {
        let removed = self._stored_allowance(owner, spender);
        self.allowances
            .at_mut(owner)
            .at_mut(spender)
            .delete_exclusive();
        removed
    }

    // Spend from a live allowance for owner and spender.
    fn _spend_allowance(
        &mut self,
        owner: &ContractAddress,
        spender: &ContractAddress,
        diff: Difference,
    ) {
        ensure(
            diff == 0 || self._allowance_is_live(owner, spender),
            ERC20Error::TooLarge,
        );
        self.allowances
            .at_mut(owner)
            .at_mut(spender)
            .amount
            .try_strict_inc(diff)
            .unwrap_or_else(|_| throw(ERC20Error::TooLarge));
    }

    fn _increase_total_supply(&mut self, amount: Amount) {
        self.total_supply
            .try_increase(amount)
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
#[contract]
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
        check_amount(amount);
        let owner = get_caller();

        self._decrease_balance(&owner, amount);
        self._lazy_increase_balance(receiver, amount);

        TransferEvent::new(&owner, receiver, amount).log();
    }

    fn transfer_from(
        &mut self,
        owner: &ContractAddress,
        receiver: &ContractAddress,
        amount: Amount,
    ) {
        let spender = get_caller();

        let diff = check_amount(amount);
        self._spend_allowance(owner, &spender, -diff);
        self._decrease_balance(owner, amount);
        self._lazy_increase_balance(receiver, amount);

        TransferEvent::new(owner, receiver, amount).log();
    }

    fn adjust_allowance(
        &mut self,
        spender: &ContractAddress,
        difference: Difference,
    ) {
        let owner = get_caller();
        self.allowances
            .at_mut(&owner)
            .at_mut(spender)
            .amount
            .try_strict_inc(difference)
            .unwrap_or_else(|_| throw(ERC20Error::TooLarge));
        AllowanceChangeEvent::new(&owner, spender, difference).log();
    }

    fn delete_allowance(&mut self, spender: &ContractAddress) {
        let owner = get_caller();
        let removed = self._remove_allowance(&owner, spender);
        AllowanceDeleteEvent::new(&owner, spender, removed).log();
    }

    fn delete_balance(&mut self) {
        let owner = get_caller();
        let mut balance = self.balances.at_mut(&owner);
        ensure(balance.get() == 0, ERC20Error::BalanceNotZero);
        balance.delete_exclusive();
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
        check_amount(amount);
        let caller = get_caller();
        ensure(
            caller == self._administrator(),
            ERC20Error::MustBeContractOwner,
        );

        // Establish the checked supply bound first. Once it holds, the lazy
        // receiver increase cannot overflow an individual balance.
        self._increase_total_supply(amount);
        self._lazy_increase_balance(receiver, amount);

        MintEvent::new(receiver, amount).log();
    }

    fn total_supply(&self) -> Amount {
        self._total_supply()
    }
}

#[contract]
impl ERC20AllowanceExpiry for TokenContract {
    fn reset_allowance(
        &mut self,
        spender: &ContractAddress,
        until: BlockHeight,
    ) {
        let owner = get_caller();
        // The delete's exclusive lock covers the expiry write.
        let removed = self._remove_allowance(&owner, spender);
        self.allowances
            .at_mut(&owner)
            .at_mut(spender)
            .expiry
            .set(&until);
        AllowanceResetEvent::new(&owner, spender, removed, until).log();
    }
}

#[cfg(feature = "metadata")]
#[contract]
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
