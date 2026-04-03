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
    non_negative_int_strict_inc(&get_balance_key(addr), diff)
}

fn add_to_allowance(
    owner: &ContractAddress,
    spender: &ContractAddress,
    diff: Difference,
) {
    non_negative_int_strict_inc(&get_allowance_key(owner, spender), diff);

    log_approval_event(owner, spender, diff);
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

    log_transfer_event(&from, to, amount);
}

#[contract(interface = "ERC20", version = "0.1.0")]
pub fn transfer_from(
    owner: &ContractAddress,
    recipient: &ContractAddress,
    amount: Amount,
) {
    let spender = get_caller();
    let diff = amount_to_diff(amount);

    add_to_allowance(owner, &spender, -diff);
    add_to_balance(owner, -diff);
    add_to_balance(recipient, diff);

    log_transfer_event(owner, recipient, amount);
}

#[contract(interface = "ERC20", version = "0.1.0")]
pub fn adjust_allowance(spender: &ContractAddress, diff: Difference) {
    let caller = get_caller();

    add_to_allowance(&caller, spender, diff)
}

#[contract(interface = "ERC20", version = "0.1.0")]
pub fn allowance(
    owner: &ContractAddress,
    spender: &ContractAddress,
) -> Amount {
    non_negative_int_try_get(&get_allowance_key(owner, spender)).unwrap_or(0)
        as Amount
}

#[contract(interface = "ERC20", version = "0.1.0")]
pub fn get_balance_of(addr: &ContractAddress) -> Amount {
    non_negative_int_try_get(&get_balance_key(addr)).unwrap_or(0) as Amount
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

    log_transfer_event(CONTRACT_ADDR_ZERO, recipient, amount);
}

#[contract(interface = "ERC20", version = "0.1.0")]
pub fn burn(loser: &ContractAddress, amount: Amount) {
    ensure_caller_is_owner();

    let diff = amount_to_diff(amount);
    add_to_balance(loser, -diff);
    add_to_total_supply(-diff);

    log_transfer_event(loser, CONTRACT_ADDR_ZERO, amount);
}

#[contract(interface = "ERC20", version = "0.1.0")]
pub fn total_supply() -> Amount {
    non_negative_int_try_get(&get_total_supply_key()).unwrap_or(0) as Amount
}

// TODO: Something better
#[cfg(feature = "dev")]
pub mod test {
    use std::rc::Rc;

    pub fn register(
        env: &Rc<::sdk::TestEnvironment>,
    ) -> ::sdk::types::Contract {
        let mut contract = env.add_contract();

        contract.register_method(
            super::ERC20Entrypoints::initialize.into(),
            super::__groundhog_exports_ERC20_initialize::initialize_internal,
        );
        contract.register_method(
            super::ERC20Entrypoints::transfer.into(),
            super::__groundhog_exports_ERC20_transfer::transfer_internal,
        );
        contract.register_method(
            super::ERC20Entrypoints::transfer_from.into(),
            super::__groundhog_exports_ERC20_transfer_from::transfer_from_internal,
        );
        contract.register_method(
            super::ERC20Entrypoints::adjust_allowance.into(),
            super::__groundhog_exports_ERC20_adjust_allowance::adjust_allowance_internal,
        );
        contract.register_method(
            super::ERC20Entrypoints::get_balance_of.into(),
            super::__groundhog_exports_ERC20_get_balance_of::get_balance_of_internal,
        );
        contract.register_method(
            super::ERC20Entrypoints::allowance.into(),
            super::__groundhog_exports_ERC20_allowance::allowance_internal,
        );
        contract.register_method(
            super::ERC20Entrypoints::get_balance.into(),
            super::__groundhog_exports_ERC20_get_balance::get_balance_internal,
        );
        contract.register_method(
            super::ERC20Entrypoints::mint.into(),
            super::__groundhog_exports_ERC20_mint::mint_internal,
        );
        contract.register_method(
            super::ERC20Entrypoints::burn.into(),
            super::__groundhog_exports_ERC20_burn::burn_internal,
        );
        contract
            .register_method(
                super::ERC20Entrypoints::total_supply.into(),
                super::__groundhog_exports_ERC20_total_supply::total_supply_internal,
            );

        contract.as_contract()
    }
}
