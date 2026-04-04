#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_package!();

use sdk::storage::{Map, Value, Vector};
use sdk::*;

#[sdk::storage]
pub struct StorageErrorContract {
    numbers: Vector<Value<u64>>,
    names: Map<u64, Value<u64>>,
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
        self.names.at(&key).try_get().unwrap_or(0)
    }
}
