//! ERC20 state and the two grant-consuming entrypoints.

use groundhog_erc20_if::log_approve_event;
use groundhog_erc20_if::{
    Amount, Difference, ERC20, ERC20Error, amount_to_diff, log_approval_event,
    log_burn_event, log_mint_event, log_transfer_event,
};
use groundhog_erc20_permit_if::{ERC20Permit, ERC20PermitError, PermitGrant};
use sdk::signer::Signed;
use sdk::storage::{Balance, Map, Value};
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

    fn _adjust_allowance(
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

        log_approval_event(owner, spender, diff);
    }

    fn _move(
        &mut self,
        owner: &ContractAddress,
        receiver: &ContractAddress,
        amount: Amount,
    ) {
        self._decrease_balance(owner, amount);
        self._lazy_increase_balance(receiver, amount);

        log_transfer_event(owner, receiver, amount);
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
        self._adjust_allowance(owner, &spender, -amount_to_diff(amount));
        self._move(owner, receiver, amount);
    }

    fn approve(&mut self, spender: &ContractAddress, amount: Amount) {
        let owner = get_caller();
        self.allowances
            .at_mut(&owner)
            .at_mut_exclusive(spender)
            .set(amount_to_diff(amount));
        log_approve_event(&owner, spender, amount);
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

        log_mint_event(receiver, amount);
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

        log_burn_event(owner, amount);
    }

    fn total_supply(&self) -> Amount {
        self.total_supply.get()
    }
}

// The owner of a grant is whoever minted it. `consume` reports that address
// from the runtime, so it is the one fact about a permit the submitter cannot
// choose; the grant's payload says the rest. Reaching this contract at all
// already proves the grant was minted for it.
#[contract(version = "0.1.0")]
impl ERC20Permit for PermitToken {
    fn permit(&mut self, grant: Signed<PermitGrant>) {
        let (grant, owner) = grant.consume();
        self._adjust_allowance(
            &owner,
            &grant.spender,
            amount_to_diff(grant.amount),
        );
    }

    fn transfer_from_signed(
        &mut self,
        grant: Signed<PermitGrant>,
        receiver: &ContractAddress,
    ) {
        let (grant, owner) = grant.consume();
        // The grant names who may spend it; the caller must be them. No
        // allowance is written, so this is the whole authorization check.
        if get_caller() != grant.spender {
            throw(ERC20PermitError::NotSpender);
        }
        self._move(&owner, receiver, grant.amount);
    }
}
