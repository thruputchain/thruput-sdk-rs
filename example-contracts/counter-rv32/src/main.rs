//! RV32 binary wrapper for the `counter` example contract.
//!
//! ## Structural shape
//!
//! Per-contract bin crate following the CKB ckb-std pattern: the
//! `counter` library stays target-agnostic, and the rv32 packaging
//! lives here.
//!
//! This crate is intentionally tiny — its job is to:
//!
//!   1. Pull in the runtime crate so its `_start`, `#[panic_handler]`,
//!      and `#[global_allocator]` items get linked.
//!   2. Pull in `counter` and expose this crate's `entrypoints` feature,
//!      which forwards to `counter/entrypoints`; `groundhog build` enables
//!      it so the `#[contract]` macro emits rv32 method entry symbols.
//!   3. Reference the symbols so DCE doesn't drop them. The linker
//!      script in `groundhog-rv32-runtime` KEEPs `.text.gh_*`, which
//!      covers it; the `extern crate counter;` line ensures the macro
//!      runs and the rlib is linked.
//!
//! The contract's macro-emitted `<method>_extern` functions live in
//! `counter::__groundhog_exports_Counter::*` (private mod) and carry
//! `#[export_name = "<16-char-hex>"]` + `#[link_section = ".text.gh_<hex>"]`
//! attributes on rv32. The lowerer reads the ELF symbol table to
//! map ContractEntrypointID → entry PC for the v3 artifact.

#![no_std]
#![no_main]

// The runtime crate provides _start, panic handler, allocator, linker
// script. Pulling it in unconditionally; it's empty on non-rv32 targets.
extern crate groundhog_rv32_runtime;

// The contract library. This crate forwards its `entrypoints` feature to
// `counter/entrypoints`; `groundhog build` enables it to pull in the
// macro-emitted method entry symbols.
extern crate counter;
