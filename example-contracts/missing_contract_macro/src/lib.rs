//! Negative fixture: a contract crate that forgot its `#[contract]` macro.
//!
//! No entrypoints are exported, but the linked interface crate still
//! contributes `Token` candidates to the raw spec section.  Historically
//! the missing-`#[contract]` guard only fired when the raw section carried
//! no interface fragments at all, so exactly this shape — broken contract
//! plus any interface-crate dependency — shipped silently as an uninvocable
//! module with an empty spec.  The guard must fire regardless of what the
//! dependency graph contributed: `groundhog-spec-tool rewrite` must error.
//! dev.sh's `check_missing_contract_macro_detected` asserts exactly that.

#![no_std]

use iface_conflict_if_a::TokenClient;
use sdk::*;

pub struct ForgotTheMacro;

// A real contract would write `#[contract] impl ForgotTheMacro { ... }`;
// this impl deliberately omits the macro, so no entrypoints are exported
// and no interface anchor is emitted.
impl ForgotTheMacro {
    pub fn ping_token(token: &ContractAddress) -> u32 {
        TokenClient::new(token).ping()
    }
}
