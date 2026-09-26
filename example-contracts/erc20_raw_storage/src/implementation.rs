//! An ERC20 implementation that manages balances via raw storage keys and syscalls
//! instead of the typed storage layer.

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
    ensure(caller == owner, ERC20Error::MustBeContractOwner);
}

fn add_to_total_supply(diff: Difference) {
    non_negative_int_strict_inc(&get_total_supply_key(), diff);
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

#[storage]
pub struct Token;

#[contract(version = "0.1.0")]
impl ERC20 for Token {
    fn initialize(&mut self, contract_owner: &ContractAddress) {
        let key = get_owner_key();

        // Make sure we haven't already been initialized
        if bytestring_exists(&key) {
            throw(ERC20Error::AlreadyInitialized);
        }

        // Store our configuration
        bytestring_set(&key, contract_owner.as_ref());
    }

    fn transfer(&mut self, to: &ContractAddress, amount: Amount) {
        let from = get_caller();
        let diff = amount_to_diff(amount);

        add_to_balance(&from, -diff);
        add_to_balance(to, diff);

        TransferEvent::new(&from, to, amount_to_diff(amount)).log();
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

        TransferEvent::new(owner, recipient, amount_to_diff(amount)).log();
    }

    fn allowance(
        &self,
        owner: &ContractAddress,
        spender: &ContractAddress,
    ) -> Amount {
        non_negative_int_try_get(&get_allowance_key(owner, spender))
            .none_if_missing()
            .unwrap_or(0) as Amount
    }

    fn adjust_allowance(
        &mut self,
        spender: &ContractAddress,
        difference: Difference,
        exclusive: bool,
    ) {
        let owner = get_caller();
        let key = get_allowance_key(&owner, spender);
        non_negative_int_try_strict_inc(&key, difference)
            .unwrap_or_else(|_| throw(ERC20Error::TooLarge));
        if exclusive {
            // Add the lock without replacing the pending delta.
            let (set, add) = sdk::sys_exn::nnint_internal::get_values(&key);
            sdk::sys_exn::nnint_internal::set_add(
                &key,
                set,
                add,
                StorageDeltaFlags::ExclusiveLock,
            );
        }
        AllowanceChangeEvent::new(&owner, spender, difference).log();
    }

    fn clear_allowance(&mut self, spender: &ContractAddress) {
        let owner = get_caller();
        let remaining = amount_to_diff(self.allowance(&owner, spender));
        delete_exclusive(&get_allowance_key(&owner, spender));
        AllowanceChangeEvent::new(&owner, spender, -remaining).log();
    }

    fn clear_balance(&mut self) {
        let key = get_balance_key(&get_caller());
        let balance = non_negative_int_try_get(&key)
            .none_if_missing()
            .unwrap_or(0);
        ensure(balance == 0, ERC20Error::BalanceNotZero);
        delete_exclusive(&key);
    }

    fn get_balance_of(&self, addr: &ContractAddress) -> Amount {
        non_negative_int_try_get(&get_balance_key(addr))
            .none_if_missing()
            .unwrap_or(0) as Amount
    }

    fn get_balance(&self) -> Amount {
        self.get_balance_of(&get_caller())
    }

    fn mint(&mut self, recipient: &ContractAddress, amount: Amount) {
        ensure_caller_is_owner();

        let diff = amount_to_diff(amount);
        add_to_balance(recipient, diff);
        add_to_total_supply(diff);

        MintEvent::new(recipient, amount_to_diff(amount)).log();
    }

    fn burn(&mut self, loser: &ContractAddress, amount: Amount) {
        ensure_caller_is_owner();

        let diff = amount_to_diff(amount);
        add_to_balance(loser, -diff);
        add_to_total_supply(-diff);

        BurnEvent::new(loser, amount_to_diff(amount)).log();
    }

    fn total_supply(&self) -> Amount {
        non_negative_int_try_get(&get_total_supply_key())
            .none_if_missing()
            .unwrap_or(0) as Amount
    }
}
