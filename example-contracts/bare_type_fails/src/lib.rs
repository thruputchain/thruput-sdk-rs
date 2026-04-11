//! Demonstrates the compile error when a custom type is used in a `#[contract]`
//! method without implementing `ContractTypeSpec`.
//!
//! To reproduce the error:
//! ```sh
//! cargo check -p bare_type_fails --features entrypoints --target wasm32-unknown-unknown
//! ```
//!
//! The spec fragment code (which requires `ContractTypeRefSpec`) is only emitted
//! under `#[cfg(all(target_family = "wasm", feature = "entrypoints"))]`, so the
//! error only surfaces when targeting wasm with the `entrypoints` feature.

#![no_std]

extern crate alloc;

use sdk::*;

#[derive(Debug, Clone, PartialEq, ReadXDR, WriteXDR)]
#[xdrs(crate = "::sdk::__xdrs")]
pub struct Payload {
    pub count: u32,
}

#[derive(Default)]
pub struct BareTypeContract;

#[contract(version = "0.1.0")]
impl BareTypeContract {
    pub fn echo(payload: Payload) -> Payload {
        payload
    }
}
