#![cfg_attr(not(feature = "dev"), no_std)]

use thruput_sdk::storage::Value;
use thruput_sdk::*;

// Same ident in both crates, no #[storage_domain] override: the default
// domain is the bare ident, so alpha::Registry and beta::Registry collide.
#[thruput_sdk::storage]
pub struct Registry {
    v: Value<u64>,
}

#[contract]
impl Registry {
    pub fn a(&self) -> u64 {
        self.v.try_get().none_if_missing().unwrap_or(0)
    }
}
