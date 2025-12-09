//! An implementation of ERC20

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
    // These internal functions use the underlying storage to create an
    // abstraction whereby all balances and allowances exist, defaulting to
    // zero if never set. They do _not_ check authorization.

    fn _init(&mut self, contract_owner: &ContractAddress) {
        self.contract_owner = contract_owner.clone();
    }

    fn _contract_owner(&self) -> &ContractAddress {
        &self.contract_owner
    }

    // Balance stored at given owner address, or zero if empty
    fn _balance_amount_of(&self, owner: &ContractAddress) -> Result<Amount> {
        let balance_cell = self.balances.get(owner)?;

        Ok(balance_cell.try_get()?.unwrap_or(0) as Amount)
    }

    // Adjust balance at given owner address, from zero if empty
    fn _adjust_balance(
        &mut self,
        owner: &ContractAddress,
        diff: Difference,
    ) -> Result<()> {
        let mut balance = self.balances.get(owner)?;

        // For never-stored-to addresses, this acts as
        // if the value was zero.  If the resulting balance
        // would be negative, this fails.
        balance.checked_inc(diff)?;
        Ok(())
    }

    fn _allowance(
        &self,
        owner: &ContractAddress,
        spender: &ContractAddress,
    ) -> Result<Amount> {
        let owner_allowances = self.allowances.get(owner)?;
        let spender_allowance = owner_allowances.get(spender)?;

        Ok(spender_allowance.try_get()?.unwrap_or(0) as Amount)
    }

    // Adjust allowance for owner and spender, from zero if empty.
    // Returns resulting allowance.
    fn _adjust_allowance(
        &mut self,
        owner: &ContractAddress,
        spender: &ContractAddress,
        diff: Difference,
    ) -> Result<()> {
        let owner_allowances = self.allowances.get(owner)?;
        let mut spender_allowance = owner_allowances.get(spender)?;

        // This fails if the result would be negative
        spender_allowance.checked_inc(diff)?;

        log_approval_event(owner, spender, diff)?;

        Ok(())
    }

    fn _adjust_total_supply(&mut self, diff: Difference) -> Result<()> {
        self.total_supply.checked_inc(diff).unwrap();
        Ok(())
    }

    fn _total_supply(&self) -> Result<Amount> {
        let supply = self.total_supply.try_get()?.unwrap_or(0) as Amount;

        Ok(supply)
    }
}

// This implements the ERC20 interface.  The `contract` macro provides
// an exported entrypoint for each method; these will be placed in a
// module with the specified name ("guest_exports").
#[contract(exports = "guest_exports", err_type = Error)]
impl ERC20 for TokenContract {
    #[init]
    fn initialize(&mut self, contract_owner: &ContractAddress) -> Result<()> {
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
        self._adjust_balance(receiver, diff)?;

        log_transfer_event(&owner, receiver, amount)?;

        Ok(())
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
        self._adjust_balance(receiver, diff)?;

        log_transfer_event(&owner, receiver, amount)?;

        Ok(())
    }

    fn adjust_allowance(
        &mut self,
        spender: &ContractAddress,
        diff: Difference,
    ) -> Result<()> {
        let owner = get_caller()?;

        self._adjust_allowance(&owner, spender, diff)
    }

    fn allowance(
        &self,
        owner: &ContractAddress,
        spender: &ContractAddress,
    ) -> Result<Amount> {
        self._allowance(owner, spender)
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
        self._adjust_total_supply(diff)?;

        log_transfer_event(CONTRACT_ADDR_ZERO, receiver, amount)?;

        Ok(())
    }

    fn burn(&mut self, owner: &ContractAddress, amount: Amount) -> Result<()> {
        let caller = get_caller()?;
        caller_ensure(
            &caller == self._contract_owner(),
            ERC20Error::MustBeContractOwner,
        )?;

        let diff = amount_to_diff(amount)?;
        self._adjust_balance(owner, -diff)?;
        self._adjust_total_supply(-diff)?;

        log_transfer_event(owner, CONTRACT_ADDR_ZERO, amount)?;

        Ok(())
    }

    fn total_supply(&self) -> Result<Amount> {
        self._total_supply()
    }
}
