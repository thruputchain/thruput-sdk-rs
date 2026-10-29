//! Interface crate "B" for the interface-metadata conflict example.
//!
//! An unrelated `trait Token` (no emits) that collides with
//! `iface_conflict_if_a`'s `Token` on the interface key — same default name
//! (trait ident) and same version (shared workspace crate version).  See
//! `iface_conflict_if_a` for the full scenario.

#![cfg_attr(not(feature = "dev"), no_std)]

use thruput_sdk::*;

#[contract]
pub trait Token {
    fn ping() -> u32;
}
