//! Raw contract (hand-written `export_name` entrypoints, no `#[contract]`)
//! that links an interface crate to call other contracts.
//!
//! The linked `Token` candidates anchor nothing here, so the spec shakes to
//! nothing.  The finalized artifact must carry NO spec sections — the same
//! shape as a raw contract whose deps contribute only typedefs (e.g.
//! `test_bytestring`) — rather than flipping to a final section claiming
//! `interfaces: []` just because a dependency carried interface candidates.

#![no_std]

use iface_conflict_if_a::TokenClient;
use sdk::*;

#[cfg_attr(target_family = "wasm", unsafe(export_name = "0"))]
pub extern "C" fn ping_token() -> i32 {
    let token = ContractAddress::new([0u8; 32]);
    let _ = TokenClient::new(&token).ping();
    0
}
