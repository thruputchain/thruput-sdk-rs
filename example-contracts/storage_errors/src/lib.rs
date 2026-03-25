#![cfg_attr(not(feature = "dev"), no_std)]

use sdk::storage::{Map, Vector};
use sdk::*;

#[sdk::storage]
pub struct StorageErrorContract {
    numbers: Vector<u64>,
    names: Map<u64, u64>,
}

/// A contract that deliberately exercises storage error paths.
/// No `throws` — errors should still propagate as LibraryExn, not Unknown.
#[contract(client = StorageErrorClient)]
impl StorageErrorContract {
    #[init]
    pub fn initialize(&mut self) {
        // Push a few items so the vector has known length
        self.numbers.push(10);
        self.numbers.push(20);
        self.numbers.push(30);
    }

    /// Access a vector element by index. Will abort with
    /// LibraryExn::Storage(IndexOutOfBounds) if index >= len.
    pub fn vector_get(&self, index: u64) -> u64 {
        self.numbers.get(index)
    }

    /// Set a vector element by index. Will abort with
    /// LibraryExn::Storage(IndexOutOfBounds) if index >= len.
    pub fn vector_set(&mut self, index: u64, value: u64) {
        self.numbers.set(index, &value);
    }

    /// Get the vector length.
    pub fn vector_len(&self) -> u64 {
        self.numbers.len()
    }

    /// Set a map entry.
    pub fn map_set(&mut self, key: u64, value: u64) {
        self.names.set(&key, &value);
    }

    /// Get a map entry. Will abort if the key doesn't exist
    /// (load_keypath fails).
    pub fn map_get(&self, key: u64) -> u64 {
        self.names.get(&key)
    }

    /// Try to get a map entry. Returns 0 if not found.
    pub fn map_try_get(&self, key: u64) -> u64 {
        self.names.try_get(&key).unwrap_or(0)
    }
}
