//! The other half of the `Split` interface, declared from a different module
//! of the same crate than `reads`. Both record the crate as their declarer,
//! and they must record the *same* one.

use thruput_sdk::*;

#[contract(interface = "Split", version = "0.1.0")]
pub fn bump(value: u32) -> u32 {
    value.wrapping_add(1)
}
