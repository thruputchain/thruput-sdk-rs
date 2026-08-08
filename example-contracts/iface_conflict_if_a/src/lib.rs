//! Interface crate "A" for the interface-metadata conflict example.
//!
//! Declares `trait Token` with `emits(PingEvent)`.  `iface_conflict_if_b`
//! declares its own unrelated `trait Token` with no emits.  Because the
//! interface name defaults to the bare trait ident and the version to the
//! crate version, both crates' candidates collide on the same interface key
//! with different emits metadata.  A caller linking both while implementing
//! neither must not fail on that disagreement (both candidates are shaken);
//! a contract that *implements* either `Token` anchors the key and must
//! still fail the spec rewrite on the ambiguous metadata.

#![cfg_attr(not(feature = "dev"), no_std)]

use sdk::*;

/// Declared by A's `Token` via `emits(...)`; absent from B's `Token`.
#[derive(Event)]
pub struct PingEvent {
    pub code: u32,
}

#[contract(emits(PingEvent))]
pub trait Token {
    fn ping() -> u32;
}
