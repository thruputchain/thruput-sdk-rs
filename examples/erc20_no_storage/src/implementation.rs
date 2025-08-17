//! An implementation of ERC20 without the storage layer.

// If the interface's `Amount` type changes away from `i128`, our casts
// below will be necessary
#![allow(clippy::unnecessary_cast)]
#![allow(clippy::useless_conversion)]

use crate::interface::*;
use sdk::*;

// Compute keys for storing things

fn get_owner_key() -> StorageKey {
    b"owner".hash().into()
}

fn get_total_supply_key() -> StorageKey {
    b"total_supply".hash().into()
}

fn get_balance_key(addr: &ContractAddress) -> StorageKey {
    (b"balance", addr).hash().into()
}

fn get_allowance_key(
    owner: &ContractAddress,
    spender: &ContractAddress,
) -> StorageKey {
    debug_assert!(owner != spender, "no allowance needed on spending self");

    (b"allowance", owner, spender).hash().into()
}

// Read/write permanent state

fn get_owner() -> Result<ContractAddress> {
    if let Some(bytes) = bytestring_get_sized(&get_owner_key())? {
        Ok(bytes.into())
    } else {
        internal_err("Unexpected bytestring size")
    }
}

fn ensure_caller_is_owner() -> Result<()> {
    let caller = get_caller()?;
    let owner = get_owner()?;
    caller_ensure(caller == owner, ERC20Error::MustBeContractOwner)
}

fn add_to_total_supply(diff: Difference) -> Result<()> {
    Ok(non_negative_int_add_default_zero(
        &get_total_supply_key(),
        diff,
    )?)
}

fn add_to_balance(addr: &ContractAddress, diff: Difference) -> Result<()> {
    Ok(non_negative_int_add_default_zero(
        &get_balance_key(addr),
        diff,
    )?)
}

fn add_to_allowance(
    owner: &ContractAddress,
    spender: &ContractAddress,
    diff: Difference,
) -> Result<()> {
    Ok(non_negative_int_add_default_zero(
        &get_allowance_key(owner, spender),
        diff,
    )?)
}

#[derive(Default)]
struct Token;

#[contract(storage = "false", err_type = Error)]
impl ERC20 for Token {
    fn initialize(
        &mut self,
        contract_owner: &ContractAddress,
    ) -> Result<()> {
        let key = get_owner_key();

        // Make sure we haven't already been initialized
        if bytestring_exists(&key)? {
            return caller_error(ERC20Error::AlreadyInitialized);
        }

        // Store ourselves
        bytestring_set(&key, contract_owner.as_ref())?;

        Ok(())
    }

    fn transfer(&mut self, to: &ContractAddress, amount: Amount) -> Result<()> {
        let caller = get_caller()?;
        let diff = amount_to_diff(amount)?;
        add_to_balance(&caller, -diff)?;
        add_to_balance(to, diff)
    }

    fn transfer_from(
        &mut self,
        owner: &ContractAddress,
        to: &ContractAddress,
        amount: Amount,
    ) -> Result<()> {
        let caller = get_caller()?;
        let diff = amount_to_diff(amount)?;
        add_to_allowance(owner, &caller, -diff)?;
        add_to_balance(owner, -diff)?;
        add_to_balance(to, diff)
    }

    fn adjust_allowance(
        &mut self,
        spender: &ContractAddress,
        diff: Difference,
    ) -> Result<()> {
        let caller = get_caller()?;

        add_to_allowance(&caller, spender, diff)
    }

    fn get_balance_of(&self, addr: &ContractAddress) -> Result<Amount> {
        Ok(non_negative_int_get_default_zero(&get_balance_key(addr))? as Amount)
    }

    fn get_balance(&self) -> Result<Amount> {
        self.get_balance_of(&get_caller()?)
    }

    fn mint(&mut self, to: &ContractAddress, amount: Amount) -> Result<()> {
        ensure_caller_is_owner()?;

        let diff = amount_to_diff(amount)?;
        add_to_balance(to, diff)?;
        add_to_total_supply(diff)
    }

    fn burn(&mut self, to: &ContractAddress, amount: Amount) -> Result<()> {
        ensure_caller_is_owner()?;

        let diff = amount_to_diff(amount)?;
        add_to_balance(to, -diff)?;
        add_to_total_supply(-diff)
    }

    fn total_supply(&self) -> Result<Amount> {
        Ok(non_negative_int_get_default_zero(&get_total_supply_key())? as Amount)
    }
}
