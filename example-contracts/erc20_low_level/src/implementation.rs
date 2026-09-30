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
    (b"allowance", owner, spender).hash().into()
}

// Read/write permanent state

fn get_owner() -> ContractAddress {
    bytestring_get_sized(&get_owner_key()).into()
}

fn ensure_caller_is_owner() {
    let caller = get_caller();
    let owner = get_owner();
    ensure(caller == owner, ERC20Error::MustBeContractOwner)
}

fn add_to_total_supply(diff: Difference) {
    non_negative_int_strict_inc(&get_total_supply_key(), diff)
}

fn add_to_balance(addr: &ContractAddress, diff: Difference) {
    // The standard error contract: a balance outside the non-negative
    // range throws ERC20Error::TooLarge. try_strict_inc reports the
    // failure instead of aborting, with no extra pre-check roundtrip.
    non_negative_int_try_strict_inc(&get_balance_key(addr), diff)
        .unwrap_or_else(|_| throw(ERC20Error::TooLarge))
}

fn add_to_allowance(
    owner: &ContractAddress,
    spender: &ContractAddress,
    diff: Difference,
) {
    non_negative_int_try_strict_inc(&get_allowance_key(owner, spender), diff)
        .unwrap_or_else(|_| throw(ERC20Error::TooLarge));
}

fn get_allowance_expiry_key(
    owner: &ContractAddress,
    spender: &ContractAddress,
) -> StorageKey {
    (b"allowance_expiry", owner, spender).hash().into()
}

// An allowance without a stored expiry never expires.
fn allowance_is_live(
    owner: &ContractAddress,
    spender: &ContractAddress,
) -> bool {
    let key = get_allowance_expiry_key(owner, spender);
    !bytestring_exists(&key)
        || *sdk::block::get_block_height()
            <= i64::from_be_bytes(bytestring_get_sized(&key))
}

// Exposed contract functions

#[contract(interface = "ERC20", version = "0.1.0")]
pub fn initialize(owner: &ContractAddress) {
    let key = get_owner_key();

    // Make sure we haven't already been initialized
    ensure(!bytestring_exists(&key), ERC20Error::AlreadyInitialized);

    // Store ourselves
    bytestring_set(&key, owner.as_ref());
}

#[contract(interface = "ERC20", version = "0.1.0")]
pub fn transfer(to: &ContractAddress, amount: Amount) {
    let from = get_caller();
    let diff = amount_to_diff(amount);

    add_to_balance(&from, -diff);
    add_to_balance(to, diff);

    TransferEvent::new(&from, to, amount_to_diff(amount)).log();
}

#[contract(interface = "ERC20", version = "0.1.0")]
pub fn transfer_from(
    owner: &ContractAddress,
    recipient: &ContractAddress,
    amount: Amount,
) {
    let spender = get_caller();
    let diff = amount_to_diff(amount);

    ensure(allowance_is_live(owner, &spender), ERC20Error::TooLarge);
    add_to_allowance(owner, &spender, -diff);
    add_to_balance(owner, -diff);
    add_to_balance(recipient, diff);

    TransferEvent::new(owner, recipient, amount_to_diff(amount)).log();
}

#[contract(interface = "ERC20", version = "0.1.0")]
pub fn adjust_allowance(spender: &ContractAddress, difference: Difference) {
    let owner = get_caller();
    let key = get_allowance_key(&owner, spender);
    non_negative_int_try_strict_inc(&key, difference)
        .unwrap_or_else(|_| throw(ERC20Error::TooLarge));
    AllowanceChangeEvent::new(&owner, spender, difference).log();
}

#[contract(interface = "ERC20", version = "0.1.0")]
pub fn reset_allowance(spender: &ContractAddress, until: BlockHeight) {
    // The delete's exclusive lock covers the expiry write.
    delete_allowance(spender);
    let key = get_allowance_expiry_key(&get_caller(), spender);
    bytestring_set(&key, &until.into_inner().to_be_bytes());
}

#[contract(interface = "ERC20", version = "0.1.0")]
pub fn delete_allowance(spender: &ContractAddress) {
    let owner = get_caller();
    let remaining = amount_to_diff(allowance(&owner, spender));
    delete_exclusive(&get_allowance_key(&owner, spender));
    delete_exclusive(&get_allowance_expiry_key(&owner, spender));
    AllowanceChangeEvent::new(&owner, spender, -remaining).log();
}

#[contract(interface = "ERC20", version = "0.1.0")]
pub fn delete_balance() {
    let key = get_balance_key(&get_caller());
    let balance = non_negative_int_try_get(&key)
        .none_if_missing()
        .unwrap_or(0);
    ensure(balance == 0, ERC20Error::BalanceNotZero);
    delete_exclusive(&key);
}

#[contract(interface = "ERC20", version = "0.1.0")]
pub fn allowance(
    owner: &ContractAddress,
    spender: &ContractAddress,
) -> Amount {
    if !allowance_is_live(owner, spender) {
        return 0;
    }
    non_negative_int_try_get(&get_allowance_key(owner, spender))
        .none_if_missing()
        .unwrap_or(0) as Amount
}

#[contract(interface = "ERC20", version = "0.1.0")]
pub fn get_balance_of(addr: &ContractAddress) -> Amount {
    non_negative_int_try_get(&get_balance_key(addr))
        .none_if_missing()
        .unwrap_or(0) as Amount
}

#[contract(interface = "ERC20", version = "0.1.0")]
pub fn get_balance() -> Amount {
    get_balance_of(&get_caller())
}

#[contract(interface = "ERC20", version = "0.1.0")]
pub fn mint(recipient: &ContractAddress, amount: Amount) {
    ensure_caller_is_owner();

    let diff = amount_to_diff(amount);
    add_to_balance(recipient, diff);
    add_to_total_supply(diff);

    MintEvent::new(recipient, amount_to_diff(amount)).log();
}

#[contract(interface = "ERC20", version = "0.1.0")]
pub fn total_supply() -> Amount {
    non_negative_int_try_get(&get_total_supply_key())
        .none_if_missing()
        .unwrap_or(0) as Amount
}
