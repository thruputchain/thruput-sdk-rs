//! # thruput-rv32-runtime
//!
//! Bare-metal runtime support for Thruput contracts targeting
//! `riscv32im-unknown-none-elf`.
//!
//! This crate is the rv32 analog of what the system + linker provide for
//! hosted targets. It supplies:
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
//! `rv32_dispatch`, which scans the dispatch table (one `Entrypoint` record
//! per method, emitted by the `#[contract]` macro into the
//! `thruput_rv32_entrypoints` section that the linker script brackets) and
//! invokes the matching wrapper. No per-method export symbols are emitted on
//! rv32.
//!
//! The target-specific runtime pieces only compile on rv32. The pure dispatch
//! primitives are also compiled for docs/tests so host unit tests can exercise
//! lookup behavior; production dependencies should remain target-gated.
//!
//! ## Stack size
//!
//! The linker script lays guest data memory out stack-first: the stack
//! occupies `[0, __stack_size)` growing down (overflow wraps below zero and
//! traps as an OOB access), statics sit above it, and the heap grows from the
//! top of statics to `__memory_size`, the end of the guest data space.
//! `__stack_size` defaults to 30K and is a hard limit. It costs no artifact
//! bytes (the lowerer trims the stack region from the stored image), only
//! address space shared with the heap.
//!
//! To override it per contract, add a `--defsym` to the contract's rv32
//! rustflags (value must be a multiple of 16; the link fails otherwise):
//!
//! ```toml
//! # .cargo/config.toml
//! [target.riscv32im-unknown-none-elf]
//! rustflags = [
//!     # ... the standard flags ...
//!     "-C", "link-arg=--defsym=__stack_size=0x8000",  # 32K stack
//! ]
//! ```
//!
//! ## Memory size
//!
//! `__memory_size` is the guest data space the VM provisions, 64K unless
//! overridden: the protocol default, and what a chain admits unless its
//! configuration raises `max_rv32_memory_bytes`. It has to agree with the
//! `memory_bytes` the artifact header declares, so it is set from one place:
//! `thruput build` reads `initial_memory_bytes` under
//! `[package.metadata.thruput]` in the contract's `Cargo.toml`, passes it
//! as `--defsym=__memory_size=<bytes>` on the link, and stamps the same value
//! into the artifact. A hand-driven build passes the same `--defsym` next to
//! `__stack_size`. The linker script `PROVIDE`s the default, so either
//! definition wins over it.

#![cfg_attr(not(test), no_std)]
// Forbid the compiler from introducing calls to builtin functions in this
// crate: mem.rs DEFINES memcpy/memmove/memset, and LLVM's loop-idiom pass
// would otherwise be allowed to lower their own loops into calls to
// themselves (infinite recursion). Encoded per-function, survives fat LTO.
#![no_builtins]

#[cfg(not(any(target_arch = "riscv32", doc, test)))]
compile_error!(
    "thruput-rv32-runtime can only be built for the riscv32 target"
);

// Pure, target-independent — compiled everywhere (host tests included).
mod dispatch;
pub use dispatch::{Entrypoint, lookup};

// Allocator gated on `alloc`, matching `thruput-sdk`'s `alloc` feature.
// rv32-only: it binds linker-script externs and installs a
// `#[global_allocator]`.
#[cfg(all(feature = "alloc", target_arch = "riscv32"))]
mod allocator;

// The allocator's pure bump arithmetic, split out so it also compiles for
// host unit tests (same pattern as dispatch.rs and mem.rs). On target it
// follows `allocator`'s gating exactly -- that module is its only caller, so
// without `alloc` it would be dead code and trip `--deny warnings`. Under
// `test` it is always compiled, so a default host `cargo test --lib` runs
// its tests without needing the feature.
#[cfg(any(all(feature = "alloc", target_arch = "riscv32"), test))]
mod bump;

// Strong mem-intrinsic overrides (rv32) + their pure copy/fill cores, which
// also compile for host unit tests (same pattern as dispatch.rs). Not
// compiled for `doc` builds — the cores are internal and would trip
// dead-code lints there.
#[cfg(any(target_arch = "riscv32", test))]
mod mem;

// rv32-only: these define `#[panic_handler]`, naked asm, and linker-script
// extern statics, none of which can compile on the host.
#[cfg(target_arch = "riscv32")]
mod panic;

#[cfg(target_arch = "riscv32")]
mod start;

#[cfg(target_arch = "riscv32")]
pub use start::_start;
