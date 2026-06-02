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
//! on the contract library AND on this runtime. The VM enters at `_start`
//! (PC 0) with the invoked method id in `a0`/`a1`; `_start` tail-calls
//! `rv32_dispatch`, which scans the `GROUNDHOG_RV32_ENTRYPOINTS` registry
//! (populated by the `#[contract]` macro, one entry per method) and invokes
//! the matching wrapper. No per-method export symbols are emitted on rv32.
//!
//! On non-rv32 targets this crate is empty — safe to depend on
//! unconditionally so contract crates can `[dependencies]` it without
//! `target.'cfg(...)'.dependencies` gymnastics.

#![cfg_attr(not(test), no_std)]

#[cfg(not(any(target_arch = "riscv32", doc, test)))]
compile_error!(
    "groundhog-rv32-runtime can only be built for the riscv32 target"
);

// Pure, target-independent — compiled everywhere (host tests included).
mod dispatch;
pub use dispatch::{Entrypoint, lookup};

// Allocator gated on `alloc`, matching `sdk`'s `alloc` feature. rv32-only:
// it binds linker-script externs and installs a `#[global_allocator]`.
#[cfg(all(feature = "alloc", target_arch = "riscv32"))]
mod allocator;

// rv32-only: these define `#[panic_handler]`, naked asm, and the linkme slice,
// none of which can compile on the host.
#[cfg(target_arch = "riscv32")]
mod panic;

#[cfg(target_arch = "riscv32")]
mod start;

#[cfg(target_arch = "riscv32")]
pub use start::{_start, GROUNDHOG_RV32_ENTRYPOINTS};
