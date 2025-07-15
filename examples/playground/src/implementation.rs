//! An implementation of ERC20

// If the interface's `Amount` type changes away from `i128`, our casts
// below will be necessary
#![allow(clippy::unnecessary_cast)]

use super::interface::{Amount, ERC20};
use sdk::storage_prelude::*;

type Balance = NNInt;
type Allowance = NNInt;

#[derive(Storage, Debug, Serialize)]
pub struct CoinContract {
    contract_owner: ContractAddress,
    balances: Map<ContractAddress, Balance>,
    allowances: Map<ContractAddress, Map<ContractAddress, Allowance>>,
    total_supply: NNInt,
    silly: bool,
    frottage: Handle<Frottage>,
    hbool1: Handle<bool>,
}

#[derive(Storage, Debug, Serialize)]
pub struct Frottage {
    touched: bool,
    count: NNInt,
    bytes: Bytestring,
}

impl CoinContract {
    // These internal functions use the underlying storage to
    // create an abstraction whereby all balances and allowances
    // exist, defaulting to zero if never set.  They also, via
    // the NNInt datatype, enforce the invariant that balances
    // and allowances are non-negative.  They do _not_ check
    // authorization.

    fn _contract_owner(&self) -> &ContractAddress {
        &self.contract_owner
    }

    // Balance stored at given owner address, or zero if empty
    fn _balance_amount_of(&self, owner: &ContractAddress) -> Result<Amount> {
        let balance = self.balances.get(owner)?;
        let amount = balance.try_get()?.unwrap_or(0) as Amount;
        Ok(amount)
    }

    // Adjust balance at given owner address, from zero if empty
    fn _adjust_balance(
        &mut self,
        owner: &ContractAddress,
        amount: Amount,
    ) -> Result<()> {
        let mut balance = self.balances.get(owner)?;

        // The NNInt datatype ensures that `add` with a negative
        // number results in a non-negative value.
        // For never-stored-to addresses, it acts as
        // if the address had contained (0, 0).
        balance.add_default_zero(amount as i128)?;
        Ok(())
    }

    // Adjust allowance for owner and spender, from zero if empty
    fn _adjust_allowance(
        &mut self,
        owner: &ContractAddress,
        spender: &ContractAddress,
        amount: Amount,
    ) -> Result<()> {
        let owner_allowances = self.allowances.get(owner)?;
        let mut spender_allowance = owner_allowances.get(spender)?;
        spender_allowance.add_default_zero(amount as i128)?;
        Ok(())
    }

    fn _adjust_total_supply(&mut self, amount: Amount) -> Result<()> {
        self.total_supply.add_default_zero(amount as i128)?;
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
#[contract(exports = "guest_exports")]
impl ERC20 for CoinContract {
    // TODO: attribute #[init]: panic if storage already present; otherwise provide default value
    fn initialize(&mut self, contract_owner: &ContractAddress) -> Result<()> {
        self.contract_owner = contract_owner.clone();

        Ok(())
    }

    fn hello(&mut self, buf: &[u8]) -> Result<u32> {
        Ok(buf.len() as u32)
    }

    // fn associated(n: u32) -> Result<u32> { Ok(n) }

    fn transfer(
        &mut self,
        receiver: &ContractAddress,
        amount: Amount,
    ) -> Result<()> {
        let owner = get_caller()?;

        self._adjust_balance(&owner, -amount)?;
        self._adjust_balance(receiver, amount)
    }

    fn transfer_from(
        &mut self,
        owner: &ContractAddress,
        receiver: &ContractAddress,
        amount: Amount,
    ) -> Result<()> {
        caller_ensure(amount >= 0, "Transfer amount must be non-negative")?;

        let spender = get_caller()?;
        self._adjust_allowance(owner, &spender, -amount)?;

        self._adjust_balance(owner, -amount)?;
        self._adjust_balance(receiver, amount)
    }

    fn adjust_allowance(
        &mut self,
        spender: &ContractAddress,
        amount: Amount,
    ) -> Result<()> {
        let owner = get_caller()?;
        self._adjust_allowance(&owner, spender, amount)
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
            "Minter must be contract owner",
        )?;
        caller_ensure(amount >= 0, "Mint amount must be non-negative")?;

        self._adjust_balance(receiver, amount)?;
        self._adjust_total_supply(amount)
    }

    fn burn(&mut self, owner: &ContractAddress, amount: Amount) -> Result<()> {
        let caller = get_caller()?;
        caller_ensure(
            &caller == self._contract_owner(),
            "Burner must be contract owner",
        )?;
        caller_ensure(amount >= 0, "Burn amount must be non-negative")?;

        self._adjust_balance(owner, -amount)?;
        self._adjust_total_supply(-amount)
    }

    fn total_supply(&self) -> Result<Amount> {
        self._total_supply()
    }

    fn add_frot(&mut self, amount: Amount) -> Result<()> {
        let mut frottage = self.frottage.load_mut()?;

        assert!(frottage.touched == false, "Shouldn't be touched");

        frottage.touched = true;
        frottage.bytes.set(&[0, 1, 2, 3, 4]);
        frottage.count.add_default_zero(amount as i128)?;

        let mut hbool1 = self.hbool1.load_mut()?;
        assert_eq!(*hbool1, false);
        *hbool1 = true;

        Ok(())
        // Drop causes flush
    }

    fn get_frot(&self) -> Result<Amount> {
        let hbool1 = self.hbool1.load()?;
        assert_eq!(*hbool1, true);

        let frottage = self.frottage.load()?;

        assert!(frottage.touched == true, "Should be touched");

        let bytes = frottage.bytes.get();
        assert!(&*bytes == &[0, 1, 2, 3, 4], "Bad bytes");

        let count = frottage.count.get()?;
        Ok(count as Amount)
    }
}

#[allow(dead_code)]
#[contract(exports = "guest_exports")]
pub fn free_function(n: u32, _addr: &ContractAddress) -> Result<u32> {
    Ok(n)
}
