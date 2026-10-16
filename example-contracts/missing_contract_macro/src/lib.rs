//! Negative fixture: a contract crate that forgot its `#[contract]` macro.
//!
//! Omitting the macro is not by itself an error — a raw contract can write
//! its `groundhog_rv32_entrypoints` records by hand (see `raw_iface_dep` and
//! `test_bytestring`).  What is an error is registering *none*, which is
//! what this crate does: it links an interface crate, so the raw spec
//! section still carries `Token` candidates, but the lowered artifact has an
//! empty dispatch table and nothing can ever call it.
//!
//! Historically this shape — broken contract plus any interface-crate
//! dependency — shipped silently as an uninvocable module with an empty
//! spec.  dev.sh's `check_rv32_missing_entrypoints_detected` asserts the
//! empty-dispatch-table guard fires on it.

#![no_std]

use iface_conflict_if_a::TokenClient;
use thruput_sdk::*;

pub struct ForgotTheMacro;

// A real contract would write `#[contract] impl ForgotTheMacro { ... }`, or
// hand-register an `Entrypoint` record. This impl deliberately does neither,
// so no entrypoint is registered and no interface anchor is emitted.
impl ForgotTheMacro {
    pub fn ping_token(token: &ContractAddress) -> u32 {
        TokenClient::new(token).ping()
    }
}
