//! Interface crate "A" for the phantom-method shaking example.
//!
//! Declares `trait Ledger { fn ping }`.  `phantom_method_if_b` declares an
//! unrelated `trait Ledger { fn steal }` that collides on the interface key
//! (same default name = trait ident, same version = shared workspace crate
//! version) with identical (empty) throws/emits metadata — so the two
//! interface candidates merge into one bucket.  A contract implementing A's
//! `Ledger` while linking B must still build, and its published spec must
//! advertise only the methods with real entrypoints: `ping`, never `steal`.

#![cfg_attr(not(feature = "dev"), no_std)]

use sdk::*;

#[contract]
pub trait Ledger {
    fn ping() -> u32;
}
