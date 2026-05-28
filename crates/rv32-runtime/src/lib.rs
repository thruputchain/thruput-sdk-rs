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
//! on the contract library AND on this runtime, and is what `cargo build
//! --target riscv32im-unknown-none-elf` produces a deployable ELF from.
//! The hogd lowerer ingests that ELF and emits a v3 artifact.
//!
//! On non-rv32 targets this crate is empty — safe to depend on
//! unconditionally so contract crates can `[dependencies]` it without
//! `target.'cfg(...)'.dependencies` gymnastics.

#![no_std]

#[cfg(target_arch = "riscv32")]
mod allocator;

#[cfg(target_arch = "riscv32")]
mod panic;

#[cfg(target_arch = "riscv32")]
mod start;

#[cfg(target_arch = "riscv32")]
pub use start::_start;

/// Emit a method entry point with the symbol naming convention the
/// hogd lowerer expects: `gh_method_<method_id>`, placed in
/// `.text.gh_method_<method_id>` so the linker script can `KEEP` it.
///
/// The wrapped function body must terminate by issuing the `Return`
/// (or `Exit`) syscall — falling off the end is `Trap::kBadPc`.
/// The macro wraps the user body in a trailing `loop {}` so the
/// `-> !` shape is satisfied without UB even if the syscall returns
/// (which it should not).
///
/// Long-term, the `#[contract]` macro should emit this directly
/// when `cfg(target_arch = "riscv32")`. This freestanding macro is
/// the spike's bridge while that work happens.
#[macro_export]
macro_rules! entry_point {
    (method_id = $id:literal, $name:ident, $body:block) => {
        #[unsafe(export_name = concat!("gh_method_", stringify!($id)))]
        #[unsafe(link_section = concat!(".text.gh_method_", stringify!($id)))]
        pub extern "C" fn $name() -> ! {
            $body
            #[allow(unreachable_code)]
            loop {
                core::hint::spin_loop();
            }
        }
    };
}
