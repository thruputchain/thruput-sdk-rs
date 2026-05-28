//! RV32 binary wrapper for the `counter` example contract.
//!
//! ## Structural shape (spike, not production)
//!
//! Per-contract bin crate that the hogd lowerer ingests as a freestanding
//! ELF. The library `counter` stays target-agnostic; the rv32 packaging
//! is owned here.
//!
//! This crate:
//!   - depends on `groundhog-rv32-runtime` for `_start`, panic, allocator,
//!     linker script;
//!   - depends on `counter` for the actual contract logic;
//!   - emits four `gh_method_<id>` symbols matching the four counter
//!     methods, via the `entry_point!` macro the runtime provides.
//!
//! ## What's deliberately stubbed
//!
//! The macro-generated `<method>_extern` symbols in `counter`
//! (`counter::__groundhog_exports_Counter::initialize_extern` etc.) are
//! inside a private module — they can't be called from this crate
//! without the `#[contract]` macro learning to either (a) emit
//! `gh_method_<id>` symbols directly on rv32, or (b) re-export the
//! `_extern` functions publicly under `cfg(target_arch = "riscv32")`.
//!
//! Until that macro work happens, each entry point here calls
//! `sdk::log(...)` to prove the full bridge ABI fires end-to-end
//! (frame build → ecall → hogd dispatch → host syscall_entry → return)
//! through a real ELF, then issues `Return` with a placeholder. The
//! structural piece — symbol naming, section placement, linker layout,
//! runtime glue — is what this sketch validates.

#![no_std]
#![no_main]

// Pull in the runtime crate so its `_start`, `#[panic_handler]`, and
// `#[global_allocator]` items are linked into the final ELF.
extern crate groundhog_rv32_runtime;

use groundhog_rv32_runtime::entry_point;

// Method IDs are placeholders. The real IDs are derived by the
// #[contract] macro from interface name + version + method name. The
// lowerer should read the (method_id, symbol) mapping from build
// metadata, not from the symbol name. Keeping numeric IDs here keeps
// the spike legible.

entry_point!(method_id = 0, gh_method_0, {
    sdk::log(b"counter-rv32: initialize");
    // TODO(spike): call counter::Counter::initialize after macro update.
    sdk::contract_op::contract_return_buf(&[]);
});

entry_point!(method_id = 1, gh_method_1, {
    sdk::log(b"counter-rv32: increment");
    // TODO(spike): call counter::Counter::increment after macro update.
    sdk::contract_op::contract_return_buf(&[]);
});

entry_point!(method_id = 2, gh_method_2, {
    sdk::log(b"counter-rv32: decrement");
    // TODO(spike): call counter::Counter::decrement after macro update.
    sdk::contract_op::contract_return_buf(&[]);
});

entry_point!(method_id = 3, gh_method_3, {
    sdk::log(b"counter-rv32: get");
    // TODO(spike): call counter::Counter::get and return its u64.
    // Placeholder: return 8 little-endian bytes for u64(0).
    let zero: u64 = 0;
    let bytes = zero.to_le_bytes();
    sdk::contract_op::contract_return_buf(&bytes);
});
