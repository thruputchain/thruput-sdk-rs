//! Interface crate for gnaw's `fixtures:logging` check.
//!
//! With `thruput-sdk/logging`, the generated client logs its arguments. Two
//! past bugs broke that only on the host:
//!
//! - The log used a by-value argument after the call had moved it.
//!   [`Logged::record`] takes [`Note`] by value, and `Note` is not `Copy`.
//! - `debug_log!` named `std` directly. This crate is `no_std` unless its
//!   `dev` feature is on, and `logging_impl` does not turn it on.

#![cfg_attr(not(feature = "dev"), no_std)]

use thruput_sdk::*;

/// Not `Copy`, so passing it by value moves it. `Debug` is unconditional:
/// the logging client formats its arguments with `{:?}`.
#[contract_type]
#[derive(Debug)]
pub struct Note {
    pub tag: u32,
}

#[contract]
pub trait Logged {
    fn record(note: Note) -> u32;
}
