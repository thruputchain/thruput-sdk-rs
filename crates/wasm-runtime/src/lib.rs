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

// `cfg_attr(not(test), no_std)` so the target-independent modules can be
// exercised by a host `cargo test --lib` (and under Miri), matching
// groundhog-rv32-runtime.
#![cfg_attr(not(test), no_std)]

#[cfg(not(any(target_arch = "wasm32", doc, test)))]
compile_error!(
    "groundhog-wasm-runtime can only be built for the wasm32 target"
);

// Allocator gated on `alloc`, matching `sdk`'s `alloc` feature. wasm-only: it
// binds the `__heap_base` linker symbol and installs a `#[global_allocator]`.
#[cfg(all(feature = "alloc", target_arch = "wasm32"))]
mod allocator;

// The allocator's pure bump arithmetic, split out so it also compiles for
// host unit tests. On target it follows `allocator`'s gating exactly -- that
// module is its only caller, so without `alloc` it would be dead code and
// trip `--deny warnings`. Under `test` it is always compiled, so a default
// host `cargo test --lib` runs its tests without needing the feature.
#[cfg(any(all(feature = "alloc", target_arch = "wasm32"), test))]
mod bump;

// wasm-only: defines `#[panic_handler]` and traps via `core::arch::wasm32`,
// neither of which can compile on the host.
#[cfg(target_arch = "wasm32")]
mod panic;
