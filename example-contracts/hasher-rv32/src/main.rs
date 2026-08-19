//! RV32 binary wrapper for the `hasher` example contract.
//!
//! Follows the `counter-rv32` pattern: the library stays target-agnostic and
//! the rv32 packaging lives here.  The `extern crate` line pulls in the
//! macro-emitted rlib so the `#[contract]` method entry symbols link in;
//! `sdk` depends on `groundhog-rv32-runtime` on rv32 targets, which supplies
//! `_start`, `#[panic_handler]`, and `#[global_allocator]` transitively.

#![no_std]
#![no_main]

extern crate hasher;
