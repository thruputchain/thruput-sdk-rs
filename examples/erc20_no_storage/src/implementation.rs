//! An implementation of ERC20 without the storage layer.

// If the interface's `Amount` type changes away from `i128`, our casts
// below will be necessary
#![allow(clippy::unnecessary_cast)]
#![allow(clippy::useless_conversion)]

use crate::interface::{Amount, ERC20, Error, Result};
use sdk::prelude::*;

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
    caller_ensure(caller == owner, Error::MustBeContractOwner)
}

fn add_to_total_supply(amount: i128) -> Result<()> {
    Ok(non_negative_int_add_default_zero(
        &get_total_supply_key(),
        amount,
    )?)
}

fn add_to_balance(addr: &ContractAddress, amount: i128) -> Result<()> {
    Ok(non_negative_int_add_default_zero(
        &get_balance_key(addr),
        amount,
    )?)
}

fn add_to_allowance(
    owner: &ContractAddress,
    spender: &ContractAddress,
    amount: i128,
) -> Result<()> {
    Ok(non_negative_int_add_default_zero(
        &get_allowance_key(owner, spender),
        amount,
    )?)
}

#[derive(Default)]
struct Token;

#[contract(storage = "false")]
impl ERC20 for Token {
    fn initialize(
        &mut self,
        contract_owner: &ContractAddress,
    ) -> Result<()> {
        let key = get_owner_key();

        // Make sure we haven't already been initialized
        if bytestring_exists(&key)? {
            return caller_error(Error::AlreadyInitialized);
        }

        // Store ourselves
        bytestring_set(&key, contract_owner.as_ref())?;

        Ok(())
    }

    fn transfer(&mut self, to: &ContractAddress, amount: Amount) -> Result<()> {
        assert!(amount <= i128::MAX as u128, "Amount too large");
        let amount = amount as i128;

        let caller = get_caller()?;

        add_to_balance(&caller, -amount)?;
        add_to_balance(to, amount)?;
        Ok(())
    }

    fn transfer_from(
        &mut self,
        owner: &ContractAddress,
        to: &ContractAddress,
        amount: Amount,
    ) -> Result<()> {
        assert!(amount <= i128::MAX as u128, "Amount too large");
        let amount = amount as i128;

        let caller = get_caller()?;
        add_to_allowance(owner, &caller, -amount)?;

        add_to_balance(owner, -amount)?;
        add_to_balance(to, amount)?;
        Ok(())
    }

    fn adjust_allowance(
        &mut self,
        spender: &ContractAddress,
        amount: Amount,
    ) -> Result<()> {
        assert!(amount <= i128::MAX as u128, "Amount too large");
        let amount = amount as i128;

        let caller = get_caller()?;

        add_to_allowance(&caller, spender, amount)?;
        Ok(())
    }

    fn get_balance_of(&self, addr: &ContractAddress) -> Result<Amount> {
        Ok(non_negative_int_get_default_zero(&get_balance_key(addr))? as Amount)
    }

    fn get_balance(&self) -> Result<Amount> {
        self.get_balance_of(&get_caller()?)
    }

    fn mint(&mut self, to: &ContractAddress, amount: Amount) -> Result<()> {
        ensure_caller_is_owner()?;

        assert!(amount <= i128::MAX as u128, "Amount too large");
        let amount = amount as i128;

        add_to_balance(to, amount)?;
        add_to_total_supply(amount)
    }

    fn burn(&mut self, to: &ContractAddress, amount: Amount) -> Result<()> {
        ensure_caller_is_owner()?;

        assert!(amount <= i128::MAX as u128, "Amount too large");
        let amount = amount as i128;

        add_to_balance(to, -amount)?;
        add_to_total_supply(-amount)
    }

    fn total_supply(&self) -> Result<Amount> {
        Ok(non_negative_int_get_default_zero(&get_total_supply_key())? as Amount)
    }
}

pub fn internal_err<T, M: AsRef<str>>(_msg: M) -> Result<T> {
    Err(Error::SystemErr)
}

pub fn caller_error<T>(err: Error) -> Result<T> {
    Err(err)
}

pub fn caller_ensure(pred: bool, err: Error) -> Result<()> {
    if pred {
        Ok(())
    } else {
        Err(err)
    }
}
