//! Raw contract (hand-written entrypoints, no `#[contract]`) that links an
//! interface crate to call other contracts.
//!
//! The linked `Token` candidates anchor nothing here, so the spec shakes to
//! nothing: a raw contract must not flip to a final spec section claiming
//! `interfaces: []` just because a dependency carried interface candidates.
//!
//! rv32 dispatches guest-side off the `groundhog_rv32_entrypoints` section,
//! which the `#[contract]` macro normally populates. This crate deliberately
//! has no macro, so it writes its record by hand — a supported shape, and the
//! one that keeps the fixture a real contract rather than a compile-only
//! stub. The dispatcher traps (ebreak) when a wrapper returns, so the
//! wrapper ends in `raw_Return`.

#![no_std]

use iface_conflict_if_a::TokenClient;
use sdk::*;

pub fn ping_token() {
    let token = ContractAddress::new([0u8; 32]);
    let _ = TokenClient::new(&token).ping();
}

#[cfg(target_arch = "riscv32")]
mod rv32_entries {
    use sdk::__rv32::Entrypoint;

    extern "C" fn ping_token() {
        super::ping_token();
        sdk::sys::raw_Return(&[], false, &[])
    }

    #[used]
    #[unsafe(link_section = "groundhog_rv32_entrypoints")]
    static PING_TOKEN: Entrypoint = Entrypoint {
        method_id: 0x0,
        wrapper: ping_token,
    };
}
