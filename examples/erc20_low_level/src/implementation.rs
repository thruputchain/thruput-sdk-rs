//! A low-level implementation of ERC20.
//!
//! (Another ERC20 implementation using the storage layer
//! can be found in `erc20`.)

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

// Exposed contract functions

#[contract(interface = "ERC20", index = 5, err_type = Error)]
#[init]
pub fn initialize(owner: &ContractAddress) -> Result<()> {
    let key = get_owner_key();

    // Make sure we haven't already been initialized
    if bytestring_exists(&key)? {
        return caller_error(ERC20Error::AlreadyInitialized);
    }

    // Store ourselves
    bytestring_set(&key, owner.as_ref())?;

    Ok(())
}

#[contract(interface = "ERC20", index = 8, err_type = Error)]
pub fn transfer(to: &ContractAddress, amount: Amount) -> Result<()> {
    let from = get_caller()?;
    let diff = amount_to_diff(amount)?;

    add_to_balance(&from, -diff)?;
    add_to_balance(to, diff)?;

    log_transfer_event(&from, to, amount)?;

    Ok(())
}

#[contract(interface = "ERC20", index = 9, err_type = Error)]
pub fn transfer_from(
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

#[contract(interface = "ERC20", index = 0, err_type = Error)]
pub fn adjust_allowance(
    spender: &ContractAddress,
    diff: Difference,
) -> Result<()> {
    let caller = get_caller()?;

    add_to_allowance(&caller, spender, diff)
}

#[contract(interface = "ERC20", index = 1, err_type = Error)]
pub fn allowance(
    owner: &ContractAddress,
    spender: &ContractAddress,
) -> Result<Amount> {
    Ok(
        non_negative_int_try_get(&get_allowance_key(owner, spender))?
            .unwrap_or(0) as Amount,
    )
}

#[contract(interface = "ERC20", index = 4, err_type = Error)]
pub fn get_balance_of(addr: &ContractAddress) -> Result<Amount> {
    Ok(
        non_negative_int_try_get(&get_balance_key(addr))?.unwrap_or(0)
            as Amount,
    )
}

#[contract(interface = "ERC20", index = 3, err_type = Error)]
pub fn get_balance() -> Result<Amount> {
    get_balance_of(&get_caller()?)
}

#[contract(interface = "ERC20", index = 6, err_type = Error)]
pub fn mint(recipient: &ContractAddress, amount: Amount) -> Result<()> {
    ensure_caller_is_owner()?;

    let diff = amount_to_diff(amount)?;
    add_to_balance(recipient, diff)?;
    add_to_total_supply(diff)?;

    log_transfer_event(&CONTRACT_ADDR_ZERO, recipient, amount)?;

    Ok(())
}

#[contract(interface = "ERC20", index = 2, err_type = Error)]
pub fn burn(loser: &ContractAddress, amount: Amount) -> Result<()> {
    ensure_caller_is_owner()?;

    let diff = amount_to_diff(amount)?;
    add_to_balance(loser, -diff)?;
    add_to_total_supply(-diff)?;

    log_transfer_event(loser, &CONTRACT_ADDR_ZERO, amount)?;

    Ok(())
}

#[contract(interface = "ERC20", index = 7, err_type = Error)]
pub fn total_supply() -> Result<Amount> {
    Ok(
        non_negative_int_try_get(&get_total_supply_key())?.unwrap_or(0)
            as Amount,
    )
}

// TODO: Something better
#[cfg(feature = "mock")]
pub mod test {
    use super::*;
    use std::rc::Rc;

    pub fn register(
        env: &Rc<::sdk::TestEnvironment>,
    ) -> ::sdk::types::Contract {
        let mut contract = env.add_contract();

        contract.register_method(INITIALIZE_EXPORT_NUM, initialize_internal);
        contract.register_method(TRANSFER_EXPORT_NUM, transfer_internal);
        contract
            .register_method(TRANSFER_FROM_EXPORT_NUM, transfer_from_internal);
        contract.register_method(
            ADJUST_ALLOWANCE_EXPORT_NUM,
            adjust_allowance_internal,
        );
        contract.register_method(
            GET_BALANCE_OF_EXPORT_NUM,
            get_balance_of_internal,
        );
        contract.register_method(ALLOWANCE_EXPORT_NUM, allowance_internal);
        contract.register_method(GET_BALANCE_EXPORT_NUM, get_balance_internal);
        contract.register_method(MINT_EXPORT_NUM, mint_internal);
        contract.register_method(BURN_EXPORT_NUM, burn_internal);
        contract
            .register_method(TOTAL_SUPPLY_EXPORT_NUM, total_supply_internal);

        contract.as_contract()
    }
}
