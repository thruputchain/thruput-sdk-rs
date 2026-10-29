//! Interface crate "B" for the phantom-method shaking example.
//!
//! An unrelated `trait Ledger` whose method `steal` carries a named payload
//! type, so the example also exercises typedef shaking of phantom-method
//! argument types.  See `phantom_method_if_a` for the full scenario.

#![cfg_attr(not(feature = "dev"), no_std)]

use thruput_sdk::*;

/// Referenced only by `steal`'s signature; must be shaken from the spec of
/// any contract where `steal` itself is shaken.
#[contract_type]
pub struct LootConfig {
    pub amount: u64,
}

#[contract]
pub trait Ledger {
    fn steal(cfg: LootConfig) -> u32;
}
