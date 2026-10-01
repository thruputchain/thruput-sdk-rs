//! RV32 binary wrapper for the `gh_entry` contract library.
//!
//! `groundhog build` builds this bin with the `entrypoints`
//! feature so the `#[contract]` macro emits rv32 method entry symbols.
//! Referencing the contract library is what links the macro-emitted symbols
//! in; the runtime (`_start`, panic handler, allocator, linker script)
//! arrives transitively through `sdk`'s rv32 runtime dependency.

#![no_std]
#![no_main]

extern crate gh_entry;
