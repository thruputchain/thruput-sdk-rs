//! # groundhog-rv32-runtime
//!
//! Bare-metal runtime support for Groundhog contracts targeting
//! `riscv32im-unknown-none-elf`.
//!
//! This crate is the rv32 analog of what the system + linker provide for
//! hosted targets, and what the WASM toolchain provides implicitly for
//! `wasm32-unknown-unknown`. It supplies:
//!
//!   - a `_start` entry symbol that the linker can resolve;
//!   - a `#[panic_handler]`;
//!   - a `#[global_allocator]` (bump, single guest memory page);
//!   - a linker script published via `build.rs` to bin crates that
//!     depend on this crate.
//!
//! Conceptual model (cf. CKB `ckb-std`): contract logic lives in a
//! target-agnostic library crate. A thin per-contract bin crate depends
//! on the contract library AND on this runtime. Building with
//! `--features entrypoints` (or via `groundhog build`) emits the
//! contract method entry symbols; the hogd lowerer ingests that ELF
//! and emits a v3 artifact.
//!
//! On non-rv32 targets this crate is empty — safe to depend on
//! unconditionally so contract crates can `[dependencies]` it without
//! `target.'cfg(...)'.dependencies` gymnastics.

#![no_std]

#[cfg(not(any(target_arch = "riscv32", doc)))]
compile_error!(
    "groundhog-rv32-runtime can only be built for the riscv32 target"
);

// Allocator gated on `alloc`, matching `sdk`'s `alloc` feature.
#[cfg(feature = "alloc")]
mod allocator;

mod panic;

mod start;

pub use start::_start;
