//! Interface crate "A" for the spec-name-reuse example.
//!
//! Declares an `Error` and an `Updated` event with the most ordinary names a
//! library could pick. `dup_name_if_b` independently picks the same two, and
//! a contract implementing both ends up with all four in one spec.

#![cfg_attr(not(feature = "dev"), no_std)]

use sdk::*;

/// Shares its ident with `dup_name_if_b::Error`. Neither crate knows the
/// other exists.
#[contract_exception]
pub enum Error {
    Denied = 1,
}

/// Shares its ident with `dup_name_if_b::Updated`.
#[derive(Event)]
pub struct Updated {
    #[topic]
    pub who: ContractAddress,
    pub limit: u32,
}

#[contract(throws(Error), emits(Updated))]
pub trait StoreA {
    fn set_limit(limit: u32) -> u32;
}
