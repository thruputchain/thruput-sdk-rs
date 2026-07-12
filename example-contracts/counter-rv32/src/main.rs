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
//!   1. Pull in `counter` and expose this crate's `entrypoints` feature,
//!      which forwards to `counter/entrypoints`; `groundhog build` enables
//!      it so the `#[contract]` macro emits rv32 method entry symbols.
//!   2. Reference the contract crate so DCE doesn't drop it. The linker
//!      script in `groundhog-rv32-runtime` KEEPs `.text.gh_*`, but the
//!      `extern crate counter;` line is what makes the macro-emitted rlib
//!      actually link in the first place.
//!   3. Host the cross-crate storage-domain uniqueness test
//!      (`tests/storage_domains.rs`): per-crate lib tests can't see
//!      sibling contract crates' storage roots, so the wrapper — the one
//!      place everything links together — asserts no two roots share a
//!      domain.
//!
//! The runtime itself (`_start`, `#[panic_handler]`, `#[global_allocator]`,
//! linker script) is no longer named here: `sdk` depends on
//! `groundhog-rv32-runtime` on rv32 targets and force-links it, so any crate
//! that depends on `sdk` — including `counter` — pulls the runtime in
//! transitively.
//!
//! The contract's macro-emitted `<method>_extern` functions live in
//! `counter::__groundhog_exports_Counter::*` (private mod) and carry
//! `#[export_name = "<16-char-hex>"]` + `#[link_section = ".text.gh_<hex>"]`
//! attributes on rv32. The lowerer reads the ELF symbol table to
//! map ContractEntrypointID → entry PC for the v3 artifact.

#![no_std]
#![no_main]

// The contract library. This crate forwards its `entrypoints` feature to
// `counter/entrypoints`; `groundhog build` enables it to pull in the
// macro-emitted method entry symbols. The runtime (_start, panic handler,
// allocator, linker script) arrives transitively via `counter` -> `sdk`.
extern crate counter;
