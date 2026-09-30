//! ERC20 state and signed grant entrypoints.

use groundhog_erc20_if::{
    AllowanceChangeEvent, Amount, Difference, ERC20, ERC20Error, MintEvent,
    TransferEvent, amount_to_diff,
};
use groundhog_erc20_permit_if::{
    DeleteAllowanceGrant, ERC20Permit, ERC20PermitError, PermitGrant,
};
use sdk::block::get_block_height;
use sdk::signer::Signed;
use sdk::storage::deletable::Deletable;
use sdk::storage::{Balance, Map, NNInt, Node, Value};
use sdk::*;

#[sdk::storage]
#[derive(sdk::Deletable)]
#[cfg_attr(feature = "dev", derive(Debug))]
pub struct Allowance {
    amount: NNInt,
    expiry: Value<BlockHeight>,
}

#[sdk::storage]
#[cfg_attr(feature = "dev", derive(Debug))]
pub struct PermitToken {
    administrator: Value<ContractAddress>,
    balances: Map<ContractAddress, Balance>,
    allowances: Map<ContractAddress, Map<ContractAddress, Node<Allowance>>>,
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

    fn _adjust_allowance(
        &mut self,
        owner: &ContractAddress,
        spender: &ContractAddress,
        diff: Difference,
    ) {
        self.allowances
            .at_mut(owner)
            .at_mut(spender)
            .amount
            .try_strict_inc(diff)
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

    fn _delete_allowance(
        &mut self,
        owner: &ContractAddress,
        spender: &ContractAddress,
    ) {
        let remaining = amount_to_diff(self.allowance(owner, spender));
        self.allowances
            .at_mut(owner)
            .at_mut(spender)
            .delete_exclusive();
        AllowanceChangeEvent::new(owner, spender, -remaining).log();
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
        ensure(
            self._allowance_is_live(owner, &spender),
            ERC20Error::TooLarge,
        );
        self._adjust_allowance(owner, &spender, -amount_to_diff(amount));
        self._move(owner, receiver, amount);
    }

    fn adjust_allowance(
        &mut self,
        spender: &ContractAddress,
        difference: Difference,
    ) {
        let owner = get_caller();
        self._adjust_allowance(&owner, spender, difference);
        AllowanceChangeEvent::new(&owner, spender, difference).log();
    }

    fn reset_allowance(
        &mut self,
        spender: &ContractAddress,
        until: BlockHeight,
    ) {
        // The delete's exclusive lock covers the expiry write.
        self.delete_allowance(spender);
        self.allowances
            .at_mut(&get_caller())
            .at_mut(spender)
            .expiry
            .set(&until);
    }

    fn delete_allowance(&mut self, spender: &ContractAddress) {
        self._delete_allowance(&get_caller(), spender);
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
        if !self._allowance_is_live(owner, spender) {
            return 0;
        }
        self.allowances
            .at(owner)
            .at(spender)
            .amount
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

    fn total_supply(&self) -> Amount {
        self.total_supply.get()
    }
}

// The owner of a grant is whoever minted it. `consume` reports that address
// from the runtime, so it is the one fact about a grant the submitter cannot
// choose; the grant's payload says the rest. Reaching this contract at all
// already proves the grant was minted for it.
#[contract(version = "0.1.0")]
impl ERC20Permit for PermitToken {
    fn transfer_from_signed(
        &mut self,
        grant: Signed<PermitGrant>,
        receiver: &ContractAddress,
        amount: Amount,
    ) {
        let (grant, owner) = grant.consume();
        // The grant names who may spend it and how much; the caller must be
        // them, asking for no more. No allowance is written, so this is the
        // whole authorization check.
        if get_caller() != grant.spender {
            throw(ERC20PermitError::NotSpender);
        }
        if amount > grant.amount {
            throw(ERC20PermitError::ExceedsGrant);
        }
        self._move(&owner, receiver, amount);
    }

    fn increase_allowance_signed(&mut self, grant: Signed<PermitGrant>) {
        let (grant, owner) = grant.consume();
        let difference = amount_to_diff(grant.amount);
        self._adjust_allowance(&owner, &grant.spender, difference);
        AllowanceChangeEvent::new(&owner, &grant.spender, difference).log();
    }

    fn delete_allowance_signed(
        &mut self,
        grant: Signed<DeleteAllowanceGrant>,
    ) {
        let (grant, owner) = grant.consume();
        self._delete_allowance(&owner, &grant.spender);
    }
}
