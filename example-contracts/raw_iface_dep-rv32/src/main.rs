//! RV32 binary wrapper for the `raw_iface_dep` example contract.
//!
//! Follows the `counter-rv32` pattern: the library stays target-agnostic and
//! the rv32 packaging lives here. The `extern crate` line pulls in the
//! hand-written `Entrypoint` records so the dispatcher can find them;
//! `sdk` depends on `groundhog-rv32-runtime` on rv32 targets, which
//! supplies `_start`, `#[panic_handler]`, and `#[global_allocator]`
//! transitively.

#![no_std]
#![no_main]

extern crate raw_iface_dep;
