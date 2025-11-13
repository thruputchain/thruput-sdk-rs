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
    Ok(non_negative_int_checked_inc(&get_total_supply_key(), diff)?)
}

fn add_to_balance(addr: &ContractAddress, diff: Difference) -> Result<()> {
    Ok(non_negative_int_checked_inc(&get_balance_key(addr), diff)?)
}

fn add_to_allowance(
    owner: &ContractAddress,
    spender: &ContractAddress,
    diff: Difference,
) -> Result<()> {
    non_negative_int_checked_inc(&get_allowance_key(owner, spender), diff)?;

    log_approval_event(owner, spender, diff)?;

    Ok(())
}

#[derive(Default)]
struct Token;

#[contract(storage = "false", err_type = Error)]
impl ERC20 for Token {
    #[init]
    fn initialize(&mut self, contract_owner: &ContractAddress) -> Result<()> {
        let key = get_owner_key();

        // Make sure we haven't already been initialized
        if bytestring_exists(&key)? {
            return caller_error(ERC20Error::AlreadyInitialized);
        }

        // Store ourselves
        bytestring_set(&key, contract_owner.as_ref())?;

        Ok(())
    }

    fn transfer(
        &mut self,
        to: &ContractAddress,
        amount: Amount,
    ) -> Result<()> {
        let from = get_caller()?;
        let diff = amount_to_diff(amount)?;

        add_to_balance(&from, -diff)?;
        add_to_balance(to, diff)?;

        log_transfer_event(&from, to, amount)?;

        Ok(())
    }

    fn transfer_from(
        &mut self,
        owner: &ContractAddress,
        recipient: &ContractAddress,
        amount: Amount,
    ) -> Result<()> {
        let spender = get_caller()?;
        let diff = amount_to_diff(amount)?;

        add_to_allowance(owner, &spender, -diff)?;
        add_to_balance(owner, -diff)?;
        add_to_balance(recipient, diff)?;

        log_transfer_event(owner, recipient, amount)?;

        Ok(())
    }

    fn allowance(
        &self,
        owner: &ContractAddress,
        spender: &ContractAddress,
    ) -> Result<Amount> {
        Ok(
            non_negative_int_try_get(&get_allowance_key(owner, spender))?
                .unwrap_or(0) as Amount,
        )
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
        Ok(
            non_negative_int_try_get(&get_balance_key(addr))?.unwrap_or(0)
                as Amount,
        )
    }

    fn get_balance(&self) -> Result<Amount> {
        self.get_balance_of(&get_caller()?)
    }

    fn mint(
        &mut self,
        recipient: &ContractAddress,
        amount: Amount,
    ) -> Result<()> {
        ensure_caller_is_owner()?;

        let diff = amount_to_diff(amount)?;
        add_to_balance(recipient, diff)?;
        add_to_total_supply(diff)?;

        log_transfer_event(&CONTRACT_ADDR_ZERO, recipient, amount)?;

        Ok(())
    }

    fn burn(&mut self, loser: &ContractAddress, amount: Amount) -> Result<()> {
        ensure_caller_is_owner()?;

        let diff = amount_to_diff(amount)?;
        add_to_balance(loser, -diff)?;
        add_to_total_supply(-diff)?;

        log_transfer_event(loser, &CONTRACT_ADDR_ZERO, amount)?;

        Ok(())
    }

    fn total_supply(&self) -> Result<Amount> {
        Ok(
            non_negative_int_try_get(&get_total_supply_key())?.unwrap_or(0)
                as Amount,
        )
    }
}
