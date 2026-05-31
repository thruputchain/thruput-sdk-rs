//! # groundhog-wasm-runtime
//!
//! Runtime support for Groundhog contracts targeting
//! `wasm32-unknown-unknown`.
//!
//! This crate is the wasm analog of [`groundhog-rv32-runtime`]. It supplies
//! the global lang items a `#![no_std]` contract needs but cannot define
//! itself without polluting the target-agnostic contract library:
//!
//!   - a `#[panic_handler]` (traps via `wasm32::unreachable`, optionally
//!     logging the panic message first — see the `panic_logging` feature);
//!   - a `#[global_allocator]` (bump, single wasm page).
//!
//! `sdk` depends on this crate under `cfg(target_family = "wasm")` and
//! force-links it (`extern crate groundhog_wasm_runtime as _;`), so contracts
//! get these items automatically — no contract or wrapper crate needs to name
//! this crate. On every non-wasm target this crate is empty.
//!
//! [`groundhog-rv32-runtime`]: ../groundhog_rv32_runtime/index.html

#![no_std]

#[cfg(target_arch = "wasm32")]
mod allocator;

#[cfg(target_arch = "wasm32")]
mod panic;
