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

/// Immediate deterministic trap.
/// Reached on an unknown method id under guest-side dispatch, or if a wrapper
/// returns (it shouldn't — wrappers diverge via the Return syscall). hogd traps
/// `ebreak` as `Trap::kBreakpoint` → deterministic failure.
#[inline(always)]
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
    // Set sp to the linker-script stack top and tail-call the dispatcher.
    // The guest owns stack placement: the VM (entry ABI v3) seeds sp = 0, a
    // deliberate trap value — any stack touch before this load wraps below
    // zero and faults OOB. The linker script parks the stack at the BOTTOM
    // of DATA (stack-first layout, so overflow traps instead of corrupting
    // the heap) and ASSERTs __stack_top is 16-byte aligned (the RISC-V ABI
    // sp alignment), so no rounding here.
    // `naked` guarantees no prologue clobbers a0/a1.
    naked_asm!(
        "la sp, __stack_top",
        "tail {dispatch}",
        dispatch = sym rv32_dispatch,
    );
}
