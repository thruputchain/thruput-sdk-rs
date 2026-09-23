//! Interface crate "B" for the spec-name-reuse example.
//!
//! Declares an `Error` and an `Updated` event with the most ordinary names a
//! library could pick. `dup_name_if_a` independently picks the same two, and
//! a contract implementing both ends up with all four in one spec.

#![cfg_attr(not(feature = "dev"), no_std)]

use sdk::*;

/// Shares its ident with `dup_name_if_a::Error`. Neither crate knows the
/// other exists.
#[contract_exception]
pub enum Error {
    Frozen = 1,
}

/// Shares its ident with `dup_name_if_a::Updated`.
#[derive(Event)]
pub struct Updated {
    #[topic]
    pub who: ContractAddress,
    pub ceiling: u64,
}

#[contract(throws(Error), emits(Updated))]
pub trait StoreB {
    fn set_ceiling(ceiling: u64) -> u64;
}
