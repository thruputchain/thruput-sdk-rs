//! RV32 binary wrapper for the big-integer benchmark contract.
//!
//! Same shape as `counter-rv32`, but in the same package as the library rather
//! than a sibling: the library is never built for wasm, so there is nothing for
//! a separate wrapper to keep target-agnostic.
//!
//! The runtime (`_start`, `#[panic_handler]`, `#[global_allocator]`, linker
//! script) arrives transitively: `sdk` force-links `groundhog-rv32-runtime` on
//! rv32 targets. The `extern crate` below is what makes the macro-emitted
//! entrypoint symbols in the library actually link -- the linker script KEEPs
//! `.text.gh_*`, but only for a crate that is in the link at all.

#![no_std]
#![no_main]

extern crate bigint_bench_contract;
