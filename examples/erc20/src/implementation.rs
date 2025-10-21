//! An implementation of ERC20

// If the interface's `Amount` type changes away from `i128`, our casts
// below will be necessary
#![allow(clippy::unnecessary_cast)]

use crate::interface::*;
use sdk::storage::{Map, NNInt};
use sdk::*;

type Balance = NNInt;
type Allowance = NNInt;

#[derive(Debug)]
#[sdk::storage]
pub struct TokenContract {
    contract_owner: ContractAddress,
    balances: Map<ContractAddress, Balance>,
    allowances: Map<ContractAddress, Map<ContractAddress, Allowance>>,
    total_supply: NNInt,
}

impl TokenContract {
    // These internal functions use the underlying storage to
    // create an abstraction whereby all balances and allowances
    // exist, defaulting to zero if never set.  They also, via
    // the NNInt datatype, enforce the invariant that balances
    // and allowances are non-negative.  They do _not_ check
    // authorization.

    fn _init(&mut self, contract_owner: &ContractAddress) {
        self.contract_owner = contract_owner.clone();
    }

    fn _contract_owner(&self) -> &ContractAddress {
        &self.contract_owner
    }

    // Balance stored at given owner address, or zero if empty
    fn _balance_amount_of(&self, owner: &ContractAddress) -> Result<Amount> {
        let balance_cell = self.balances.get(owner)?;
        let amount = balance_cell.try_get()?.unwrap_or(0) as Amount;
        Ok(amount)
    }

    // Adjust balance at given owner address, from zero if empty
    fn _adjust_balance(
        &mut self,
        owner: &ContractAddress,
        diff: Difference,
    ) -> Result<()> {
        let mut balance = self.balances.get(owner)?;

        // The NNInt datatype ensures that `add` with a negative
        // number results in a non-negative value.
        // For never-stored-to addresses, it acts as
        // if the address had contained (0, 0).
        balance.add_default_zero(diff)?;
        Ok(())
    }

    // Adjust allowance for owner and spender, from zero if empty
    fn _adjust_allowance(
        &mut self,
        owner: &ContractAddress,
        spender: &ContractAddress,
        diff: Difference,
    ) -> Result<()> {
        let owner_allowances = self.allowances.get(owner)?;
        let mut spender_allowance = owner_allowances.get(spender)?;
        spender_allowance.add_default_zero(diff)?;
        Ok(())
    }

    fn _adjust_total_supply(&mut self, diff: Difference) -> Result<()> {
        self.total_supply.add_default_zero(diff)?;
        Ok(())
    }

    fn _total_supply(&self) -> Result<Amount> {
        let amount = self.total_supply.try_get()?.unwrap_or(0) as Amount;
        Ok(amount)
    }
}

// This implements the ERC20 interface.  The `contract` macro provides
// an exported entrypoint for each method; these will be placed in a
// module with the specified name ("guest_exports").
#[contract(exports = "guest_exports", err_type = Error)]
impl ERC20 for TokenContract {
    #[init]
    fn initialize(&mut self, contract_owner: &ContractAddress) -> Result<()> {
        // contract_abort(&Error::MustBeContractOwner);

        self._init(contract_owner);
        Ok(())
    }

    fn transfer(
        &mut self,
        receiver: &ContractAddress,
        amount: Amount,
    ) -> Result<()> {
        let owner = get_caller()?;

        let diff = amount_to_diff(amount)?;
        self._adjust_balance(&owner, -diff)?;
        self._adjust_balance(receiver, diff)
    }

    fn transfer_from(
        &mut self,
        owner: &ContractAddress,
        receiver: &ContractAddress,
        amount: Amount,
    ) -> Result<()> {
        let spender = get_caller()?;

        let diff = amount_to_diff(amount)?;
        self._adjust_allowance(owner, &spender, -diff)?;
        self._adjust_balance(owner, -diff)?;
        self._adjust_balance(receiver, diff)
    }

    fn adjust_allowance(
        &mut self,
        spender: &ContractAddress,
        diff: Difference,
    ) -> Result<()> {
        let owner = get_caller()?;

        self._adjust_allowance(&owner, spender, diff)
    }

    fn get_balance_of(&self, owner: &ContractAddress) -> Result<Amount> {
        self._balance_amount_of(owner)
    }

    fn get_balance(&self) -> Result<Amount> {
        let owner = get_caller()?;
        self._balance_amount_of(&owner)
    }

    fn mint(
        &mut self,
        receiver: &ContractAddress,
        amount: Amount,
    ) -> Result<()> {
        let caller = get_caller()?;
        caller_ensure(
            &caller == self._contract_owner(),
            ERC20Error::MustBeContractOwner,
        )?;

        let diff = amount_to_diff(amount)?;
        self._adjust_balance(receiver, diff)?;
        self._adjust_total_supply(diff)
    }

    fn burn(&mut self, owner: &ContractAddress, amount: Amount) -> Result<()> {
        let caller = get_caller()?;
        caller_ensure(
            &caller == self._contract_owner(),
            ERC20Error::MustBeContractOwner,
        )?;

        let diff = amount_to_diff(amount)?;
        self._adjust_balance(owner, -diff)?;
        self._adjust_total_supply(-diff)
    }

    fn total_supply(&self) -> Result<Amount> {
        self._total_supply()
    }
}
