//! Raw contract (hand-written entrypoints, no `#[contract]`) that links an
//! interface crate to call other contracts.
//!
//! The linked `Token` candidates anchor nothing here, so the spec shakes to
//! nothing: a raw contract must not flip to a final spec section claiming
//! `interfaces: []` just because a dependency carried interface candidates.
//!
//! Compile-only fixture today. Its entrypoints were rv32-ported by nobody —
//! guest dispatch goes through the `Entrypoint` records the `#[contract]`
//! macro emits, and this crate deliberately has no macro — so it carries no
//! rv32 bin wrapper and the artifact-shape assertion it used to back has no
//! rv32 equivalent. Building it (host and under the rv32 contract lint) still
//! pins that the raw syscall surface compiles.

#![no_std]

use iface_conflict_if_a::TokenClient;
use sdk::*;

pub extern "C" fn ping_token() -> i32 {
    let token = ContractAddress::new([0u8; 32]);
    let _ = TokenClient::new(&token).ping();
    0
}
