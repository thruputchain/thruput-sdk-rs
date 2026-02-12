//! An implementation of ERC20 without the storage layer.

// If the interface's `Amount` type changes away from `i128`, our casts
// below will be necessary
#![allow(clippy::unnecessary_cast)]
#![allow(clippy::useless_conversion)]

use crate::interface::*;
use sdk::*;
// Override with the non-result version:
use sdk::asys::bytestring_get_sized;

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
    ensure(caller == owner, ERC20Error::MustBeContractOwner);
}

fn add_to_total_supply(diff: Difference) {
    non_negative_int_strict_inc(&get_total_supply_key(), diff);
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

#[derive(Default)]
struct Token;

#[contract(storage = "false", throws = ERC20Error, version = "0.2.0")]
impl ERC20 for Token {
    #[init]
    fn initialize(&mut self, contract_owner: &ContractAddress) {
        let key = get_owner_key();

        // Make sure we haven't already been initialized
        if asys::bytestring_exists(&key) {
            abort(&ERC20Error::AlreadyInitialized);
        }

        // Store our configuration
        asys::bytestring_set(&key, contract_owner.as_ref());
    }

    fn transfer(
        &mut self,
        to: &ContractAddress,
        amount: Amount,
    ) {
        let from = get_caller();
        let diff = amount_to_diff(amount);

        add_to_balance(&from, -diff);
        add_to_balance(to, diff);

        log_transfer_event(&from, to, amount);
    }

    fn transfer_from(
        &mut self,
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

    fn allowance(
        &self,
        owner: &ContractAddress,
        spender: &ContractAddress,
    ) -> Amount {
        non_negative_int_try_get(&get_allowance_key(owner, spender))
            .unwrap_or(0) as Amount
    }

    fn adjust_allowance(
        &mut self,
        spender: &ContractAddress,
        diff: Difference,
    ) {
        let caller = get_caller();

        add_to_allowance(&caller, spender, diff);
    }

    fn get_balance_of(&self, addr: &ContractAddress) -> Amount {
        non_negative_int_try_get(&get_balance_key(addr)).unwrap_or(0)
            as Amount
    }

    fn get_balance(&self) -> Amount {
        self.get_balance_of(&get_caller())
    }

    fn mint(
        &mut self,
        recipient: &ContractAddress,
        amount: Amount,
    ) {
        ensure_caller_is_owner();

        let diff = amount_to_diff(amount);
        add_to_balance(recipient, diff);
        add_to_total_supply(diff);

        log_transfer_event(&CONTRACT_ADDR_ZERO, recipient, amount);
    }

    fn burn(&mut self, loser: &ContractAddress, amount: Amount) {
        ensure_caller_is_owner();

        let diff = amount_to_diff(amount);
        add_to_balance(loser, -diff);
        add_to_total_supply(-diff);

        log_transfer_event(loser, &CONTRACT_ADDR_ZERO, amount);
    }

    fn total_supply(&self) -> Amount {
        non_negative_int_try_get(&get_total_supply_key()).unwrap_or(0)
            as Amount
    }
}
