#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();

pub use sdk::storage::*;
pub use sdk::*;

/// Regression fixture: slot pins override declaration order.
///
/// Declaration order: first(slot=1), second(slot=0), third(slot=2, positional default).
/// Storage layout: slot 0 → `second`, slot 1 → `first`, slot 2 → `third`.
#[storage]
pub struct SlotReorder {
    #[slot(1)]
    pub first: Value<u64>,
    #[slot(0)]
    pub second: Value<u64>,
    pub third: Value<u64>,
}

#[contract]
impl SlotReorder {
    pub fn set_first(&mut self, v: u64) {
        self.first.set(&v);
    }
    pub fn set_second(&mut self, v: u64) {
        self.second.set(&v);
    }
    pub fn set_third(&mut self, v: u64) {
        self.third.set(&v);
    }
    pub fn get_first(&self) -> u64 {
        self.first.get()
    }
    pub fn get_second(&self) -> u64 {
        self.second.get()
    }
    pub fn get_third(&self) -> u64 {
        self.third.get()
    }
}
