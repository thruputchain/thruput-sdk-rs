#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();

use sdk::storage::{Map, Value, Vector};
use sdk::*;

#[sdk::storage]
pub struct StorageErrorContract {
    numbers: Vector<Value<u64>>,
    names: Map<u64, Value<u64>>,
    plain: Value<u64>,
}

/// A contract that deliberately exercises storage error paths.
/// No `throws` — errors should still propagate as LibraryExn, not Unknown.
#[contract(client = StorageErrorClient)]
impl StorageErrorContract {
    pub fn initialize(&mut self) {
        // Push a few items so the vector has known length
        self.numbers.push_with(|slot| slot.set(&10));
        self.numbers.push_with(|slot| slot.set(&20));
        self.numbers.push_with(|slot| slot.set(&30));
        self.plain.set(&0);
    }

    /// Access a vector element by index. Will abort with
    /// LibraryExn::Storage(IndexOutOfBounds) if index >= len.
    pub fn vector_get(&self, index: u32) -> u64 {
        self.numbers.at(index).get()
    }

    /// Set a vector element by index. Will abort with
    /// LibraryExn::Storage(IndexOutOfBounds) if index >= len.
    pub fn vector_set(&mut self, index: u32, value: u64) {
        self.numbers.at_mut(index).set(&value);
    }

    /// Get the vector length.
    pub fn vector_len(&self) -> u32 {
        self.numbers.len()
    }

    /// Deliberately write an undecodable payload (zero bytes) at the
    /// vector's len slot (fixed child 0 of the vector's keypath) to
    /// simulate slot aliasing / type confusion.
    pub fn corrupt_len_slot(&mut self) {
        let mut bad: Value<()> =
            Value::from(self.numbers.keypath().child(&0u32));
        bad.set(&());
    }

    /// Set a map entry.
    pub fn map_set(&mut self, key: u64, value: u64) {
        self.names.at_mut(&key).set(&value);
    }

    /// Get a map entry. Will abort if the key doesn't exist
    /// (load_keypath fails).
    pub fn map_get(&self, key: u64) -> u64 {
        self.names.at(&key).get()
    }

    /// Try to get a map entry. Returns 0 if not found.
    pub fn map_try_get(&self, key: u64) -> u64 {
        self.names.at(&key).try_get().none_if_missing().unwrap_or(0)
    }

    /// Early-exit while a deferred-write guard is live: on a real guest the
    /// Return syscall skips Drop and the write to `plain` is silently lost.
    /// The native live-guard check must fail this loudly instead.
    pub fn early_return_with_live_guard(&mut self) {
        let mut guard = self.plain.load_mut();
        *guard += 1;
        sdk::internal::contract_return_encoded(&());
    }

    /// Holding a guard across a nested call is legitimate (the callee frame
    /// has its own state); must NOT false-positive the callee's return.
    pub fn guard_across_call(&mut self, other: ContractAddress) -> u64 {
        let mut guard = self.plain.load_mut();
        *guard += 1;
        let peer_len = StorageErrorClient::new(&other).vector_len();
        *guard += peer_len as u64;
        *guard
        // guard drops here -> flush; wrapper then returns normally
    }

    pub fn get_plain(&self) -> u64 {
        self.plain.try_get().none_if_missing().unwrap_or(0)
    }
}
