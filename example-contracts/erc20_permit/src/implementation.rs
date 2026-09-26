//! ERC20 state and signed transfers.

use groundhog_erc20_if::*;
use groundhog_erc20_permit_if::{ERC20Permit, ERC20PermitError, PermitGrant};
use sdk::signer::Signed;
use sdk::storage::{Balance, Deletable, Map, Value};
use sdk::*;

type Allowance = sdk::storage::NNInt;

#[sdk::storage]
#[cfg_attr(feature = "dev", derive(Debug))]
pub struct PermitToken {
    administrator: Value<ContractAddress>,
    balances: Map<ContractAddress, Balance>,
    allowances: Map<ContractAddress, Map<ContractAddress, Allowance>>,
    total_supply: Balance,
}

impl PermitToken {
    // Internal helpers. None of these check authorization.

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

    fn _spend_allowance(
        &mut self,
        owner: &ContractAddress,
        spender: &ContractAddress,
        diff: Difference,
    ) {
        let mut owner_allowances = self.allowances.at_mut(owner);
        let mut allowance = owner_allowances.at_mut(spender);
        allowance
            .try_strict_inc(diff)
            .unwrap_or_else(|_| throw(ERC20Error::TooLarge));
    }

    fn _move(
        &mut self,
        owner: &ContractAddress,
        receiver: &ContractAddress,
        amount: Amount,
    ) {
        self._decrease_balance(owner, amount);
        self._lazy_increase_balance(receiver, amount);

        TransferEvent::new(owner, receiver, amount_to_diff(amount)).log();
    }
}

#[contract(version = "0.1.0")]
impl ERC20 for PermitToken {
    fn initialize(&mut self, administrator: &ContractAddress) {
        if self.administrator.try_get().none_if_missing().is_some() {
            throw(ERC20Error::AlreadyInitialized);
        }
        self.administrator.set(administrator);
    }

    fn transfer(&mut self, receiver: &ContractAddress, amount: Amount) {
        let owner = get_caller();
        self._move(&owner, receiver, amount);
    }

    fn transfer_from(
        &mut self,
        owner: &ContractAddress,
        receiver: &ContractAddress,
        amount: Amount,
    ) {
        let spender = get_caller();
        self._spend_allowance(owner, &spender, -amount_to_diff(amount));
        self._move(owner, receiver, amount);
    }

    fn adjust_allowance(
        &mut self,
        spender: &ContractAddress,
        difference: Difference,
        exclusive: bool,
    ) {
        let owner = get_caller();
        let mut allowances = self.allowances.at_mut(&owner);
        let mut allowance = if exclusive {
            allowances.at_mut_exclusive(spender)
        } else {
            allowances.at_mut(spender)
        };
        allowance
            .try_strict_inc(difference)
            .unwrap_or_else(|_| throw(ERC20Error::TooLarge));
        AllowanceChangeEvent::new(&owner, spender, difference).log();
    }

    fn clear_allowance(&mut self, spender: &ContractAddress) {
        let owner = get_caller();
        let remaining = amount_to_diff(self.allowance(&owner, spender));
        self.allowances
            .at_mut(&owner)
            .at_mut(spender)
            .delete_exclusive();
        AllowanceChangeEvent::new(&owner, spender, -remaining).log();
    }

    fn clear_balance(&mut self) {
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
        self.allowances
            .at(owner)
            .at(spender)
            .try_get()
            .none_if_missing()
            .unwrap_or(0) as Amount
    }

    fn get_balance_of(&self, addr: &ContractAddress) -> Amount {
        self.balances.at(addr).get()
    }

    fn get_balance(&self) -> Amount {
        let owner = get_caller();
        self.balances.at(&owner).get()
    }

    fn mint(&mut self, receiver: &ContractAddress, amount: Amount) {
        ensure(
            get_caller() == self.administrator.get(),
            ERC20Error::MustBeContractOwner,
        );
        // Establish the checked supply bound first. Once it holds, the lazy
        // receiver increase cannot overflow an individual balance.
        self.total_supply
            .try_increase(amount)
            .unwrap_or_else(|_| throw(ERC20Error::TooLarge));
        self._lazy_increase_balance(receiver, amount);

        MintEvent::new(receiver, amount_to_diff(amount)).log();
    }

    fn burn(&mut self, owner: &ContractAddress, amount: Amount) {
        ensure(
            get_caller() == self.administrator.get(),
            ERC20Error::MustBeContractOwner,
        );
        self._decrease_balance(owner, amount);
        self.total_supply
            .try_decrease(amount)
            .unwrap_or_else(|_| throw(ERC20Error::TooLarge));

        BurnEvent::new(owner, amount_to_diff(amount)).log();
    }

    fn total_supply(&self) -> Amount {
        self.total_supply.get()
    }
}

// Successful consumption verifies the consumer and returns the minter.
#[contract(version = "0.1.0")]
impl ERC20Permit for PermitToken {
    fn transfer_from_signed(
        &mut self,
        grant: Signed<PermitGrant>,
        receiver: &ContractAddress,
    ) {
        let (grant, owner) = grant.consume();
        if get_caller() != grant.spender {
            throw(ERC20PermitError::NotSpender);
        }
        self._move(&owner, receiver, grant.amount);
    }
}
