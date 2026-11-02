//! Regression fixture: user parameters named after macro-generated locals
//! (`entrypoint`, `args`, `state`, `state_node`) must be passed through
//! verbatim, not shadowed by the generated bindings (audit finding #5:
//! a client-side param named `entrypoint` silently sent the generated
//! entrypoint ID instead of the argument).
#![cfg_attr(not(feature = "dev"), no_std)]

thruput_sdk::declare_mock_contract!();

use thruput_sdk::storage::Value;
use thruput_sdk::*;

#[derive(Debug)]
#[thruput_sdk::storage]
pub struct Hygiene {
    marker: Value<u64>,
}

#[contract(client = HygieneClient)]
impl Hygiene {
    /// Client-side hazard: generated method binds the entrypoint ID before
    /// capturing user args.
    pub fn echo_entrypoint(&self, entrypoint: u64) -> u64 {
        entrypoint
    }

    /// Client-side: generated method binds the args tuple.
    pub fn echo_args(&self, args: u64) -> u64 {
        args
    }

    /// Guest-side hazard: generated wrapper binds the loaded state.
    pub fn echo_state(&self, state: u64) -> u64 {
        state
    }

    /// Guest-side: generated wrapper binds the state node.
    pub fn echo_state_node(&self, state_node: u64) -> u64 {
        state_node
    }

    /// All at once, mixed with a real storage read so `self` is used.
    pub fn echo_sum(&self, state: u64, entrypoint: u64, args: u64) -> u64 {
        state
            + entrypoint
            + args
            + self.marker.try_get().none_if_missing().unwrap_or(0)
    }
}
