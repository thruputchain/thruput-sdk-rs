//! One interface, declared from two modules of one crate.
//!
//! `#[contract]` on a free function or a bare impl records the *crate* as the
//! interface's declarer, not a trait, because a crate may spread one
//! interface over several blocks. `erc20_low_level` already does that with
//! ten blocks — but all of them sit in one module, so a crate identity that
//! varied by module would still look consistent there.
//!
//! This contract splits the same interface across a module boundary, which is
//! what that cannot catch. If the crate's declarer id were module-scoped,
//! these two halves would read as two declarers under one interface key and
//! canonicalization would reject the contract for spreading its own interface
//! across its own modules.

#![cfg_attr(not(feature = "dev"), no_std)]

thruput_sdk::declare_mock_contract!();

pub mod reads;
pub mod writes;
