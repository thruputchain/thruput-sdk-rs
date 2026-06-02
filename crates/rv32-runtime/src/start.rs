//! rv32 `_start`: the guest dispatch entry.
//!
//! The hogd VM enters every call at PC 0 (`_start`) with the invoked
//! `method_id` split across `a0` (low 32) and `a1` (high 32). `_start` is a
//! naked shim: it sets `sp` and tail-calls `rv32_dispatch` WITHOUT touching
//! `a0`/`a1`, so the RISC-V C ABI reassembles them into the `u64` argument for
//! free. The dispatcher scans the linkme registry populated by the
//! `#[contract]` macro and calls the matching method wrapper.

use crate::dispatch::{Entrypoint, lookup};
use core::arch::{asm, naked_asm};
use linkme::distributed_slice;

/// Registry of every contract method, populated at link time by the
/// `#[contract]` macro (one registry element per method, each emitted as a
/// `#[distributed_slice(GROUNDHOG_RV32_ENTRYPOINTS)] static`).
#[distributed_slice]
pub static GROUNDHOG_RV32_ENTRYPOINTS: [Entrypoint];

/// Immediate, deterministic trap.
/// Reached on an unknown method id (the Decision-2 default arm / host-filtered
/// backstop) or if a wrapper returns (it shouldn't — wrappers diverge via the
/// Return syscall). hogd traps `ebreak` as `Trap::kBreakpoint` → deterministic
/// failure.
fn deterministic_trap() -> ! {
    unsafe { asm!("ebreak", options(noreturn)) }
}

/// C-ABI entry called by the naked `_start`. `method_id` arrives reassembled
/// from the seeded `a0`/`a1`.
extern "C" fn rv32_dispatch(method_id: u64) -> ! {
    if let Some(wrapper) = lookup(&GROUNDHOG_RV32_ENTRYPOINTS, method_id) {
        wrapper();
    }
    deterministic_trap()
}

#[unsafe(naked)]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text._start")]
pub extern "C" fn _start() -> ! {
    // Set sp to the top of guest memory (16-byte aligned) and tail-call the
    // dispatcher. The VM also seeds sp in Machine::reset(); we set it
    // defensively so a real ELF entry is safe to call from a stale stack.
    // `naked` guarantees no prologue clobbers a0/a1.
    naked_asm!(
        "la sp, __stack_top",
        "andi sp, sp, -16",
        "tail {dispatch}",
        dispatch = sym rv32_dispatch,
    );
}
