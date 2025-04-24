//! A low-level implementation of ERC20.
//!
//! (Another ERC20 implementation using the storage layer
//! can be found in `erc20`.)

// If the interface's `Amount` type changes away from `i128`, our casts
// below will be necessary
#![allow(clippy::unnecessary_cast)]
#![allow(clippy::useless_conversion)]

use crate::interface::Amount;
use sdk::prelude::*;
use sdk::result::{app_err, ensure, Result}; // TODO: contract-specific error type

// Compute keys for storing things

fn get_owner_key() -> StorageKey {
    b"owner".hash()
}

fn get_total_supply_key() -> StorageKey {
    b"total_supply".hash()
}

fn get_balance_key(addr: &ContractAddress) -> StorageKey {
    (b"balance", addr).hash()
}

fn get_allowance_key(
    owner: &ContractAddress,
    spender: &ContractAddress,
) -> StorageKey {
    debug_assert!(owner != spender, "no allowance needed on spending self");

    (b"allowance", owner, spender).hash()
}

// Read/write permanent state

fn get_owner() -> Result<ContractAddress> {
    if let Some(bytes) = bytestring_get_sized(&get_owner_key())? {
        Ok(bytes.into())
    } else {
        app_err("Unexpected bytestring size".into())
    }
}

fn ensure_caller_is_owner() -> Result<()> {
    let caller = get_caller()?;
    let owner = get_owner()?;
    ensure(caller == owner, "Caller must be owner")
}

fn add_to_total_supply(amount: Amount) -> Result<()> {
    Ok(non_negative_int_add_default_zero(
            &get_total_supply_key(),
            amount as i128,
            )?)
}

fn add_to_balance(addr: &ContractAddress, amount: Amount) -> Result<()> {
    Ok(non_negative_int_add_default_zero(
            &get_balance_key(addr),
            amount as i128,
            )?)
}

fn add_to_allowance(
    owner: &ContractAddress,
    spender: &ContractAddress,
    amount: Amount,
) -> Result<()> {
    Ok(non_negative_int_add_default_zero(
            &get_allowance_key(owner, spender),
            amount as i128,
            )?)
}

// Exposed contract functions

#[contract]
pub fn initialize(owner: &ContractAddress) -> Result<()> {
    let key = get_owner_key();

    // Make sure we haven't already been initialized
    if bytestring_exists(&key)? {
        panic!("Contract already initialized");
    }

    // Store ourselves
    bytestring_set(&key, owner.as_slice())?;

    Ok(())
}

#[contract]
pub fn transfer(
    to: &ContractAddress,
    amount: Amount,
) -> Result<()> {
    ensure(amount >= 0, "no negative spends allowed")?;

    let caller = get_caller()?;

    add_to_balance(&caller, -amount)?;
    add_to_balance(to, amount)?;
    Ok(())
}

#[contract]
pub fn transfer_from(
    owner: &ContractAddress,
    to: &ContractAddress,
    amount: Amount,
) -> Result<()> {
    ensure(amount >= 0, "no negative spends allowed")?;

    let caller = get_caller()?;
    add_to_allowance(owner, &caller, -amount)?;

    add_to_balance(owner, -amount)?;
    add_to_balance(to, amount)?;
    Ok(())
}

#[contract]
pub fn adjust_allowance(
    spender: &ContractAddress,
    amount: Amount,
) -> Result<()> {
    let caller = get_caller()?;

    add_to_allowance(&caller, spender, amount)?;
    Ok(())
}

#[contract]
pub fn get_balance_of(addr: &ContractAddress) -> Result<Amount> {
    Ok(non_negative_int_get_default_zero(&get_balance_key(addr))? as Amount)
}

#[contract]
pub fn get_balance() -> Result<Amount> {
    get_balance_of(&get_caller()?)
}

#[contract]
pub fn mint(to: &ContractAddress, amount: Amount) -> Result<()> {
    ensure_caller_is_owner()?;
    ensure(amount >= 0, "mint amount must be nonnegative")?;

    add_to_balance(to, amount)?;
    add_to_total_supply(amount)
}

#[contract]
pub fn burn(to: &ContractAddress, amount: Amount) -> Result<()> {
    ensure_caller_is_owner()?;
    ensure(amount >= 0, "burn amount must be nonnegative")?;

    add_to_balance(to, -amount)?;
    add_to_total_supply(-amount)
}

#[contract]
pub fn total_supply() -> Result<Amount> {
    Ok(non_negative_int_get_default_zero(&get_total_supply_key())? as Amount)
}

#[cfg(feature="mock")]
pub fn register(env : &mut ::sdk::TestEnvironment) -> ::sdk::types::Contract {
    let mut contract = env.add_contract();

    contract.register_method(INITIALIZE_EXPORT_NUM, initialize_internal);
    contract.register_method(TRANSFER_EXPORT_NUM, transfer_internal);
    contract.register_method(TRANSFER_FROM_EXPORT_NUM, transfer_from_internal);
    contract.register_method(ADJUST_ALLOWANCE_EXPORT_NUM, adjust_allowance_internal);
    contract.register_method(GET_BALANCE_OF_EXPORT_NUM, get_balance_of_internal);
    contract.register_method(GET_BALANCE_EXPORT_NUM, get_balance_internal);
    contract.register_method(MINT_EXPORT_NUM, mint_internal);
    contract.register_method(BURN_EXPORT_NUM, burn_internal);
    contract.register_method(TOTAL_SUPPLY_EXPORT_NUM, total_supply_internal);

    contract.as_contract()
}
