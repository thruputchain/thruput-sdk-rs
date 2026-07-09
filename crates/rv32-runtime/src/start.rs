//! rv32 `_start`: the guest dispatch entry.
//!
//! The hogd VM enters every call at PC 0 (`_start`) with the invoked
//! `method_id` split across `a0` (low 32) and `a1` (high 32). `_start` is a
//! naked shim: it sets `sp` and tail-calls `rv32_dispatch` WITHOUT touching
//! `a0`/`a1`, so the RISC-V C ABI reassembles them into the `u64` argument for
//! free. The dispatcher scans the dispatch table populated by the
//! `#[contract]` macro (one `Entrypoint` record per method, emitted into the
//! `groundhog_rv32_entrypoints` section) and calls the matching wrapper.

use crate::dispatch::{Entrypoint, lookup};
use core::arch::{asm, naked_asm};

// Dispatch-table bracket symbols. The linker script
// (link/groundhog-rv32.ld) defines both around the KEEP'd
// `groundhog_rv32_entrypoints` input section. Declared as `Entrypoint` so
// `&raw const` yields correctly-typed table pointers; the "values" are never
// read directly, only their addresses.
unsafe extern "C" {
    static __groundhog_rv32_entrypoints_start: Entrypoint;
    static __groundhog_rv32_entrypoints_end: Entrypoint;
}

/// The dispatch table, reconstructed from the linker-script bracket symbols.
///
/// Deliberately NOT a linkme `distributed_slice`: linkme's `Deref` re-runs a
/// duplicate-definition check on every access, which LLVM cannot fold (the
/// bracket symbols are link-time constants but compile-time opaque) — that
/// cost ~28 B and ~6-7 executed instructions on every VM entry.
fn entrypoints() -> &'static [Entrypoint] {
    // Length via address arithmetic, not `offset_from`: the two statics are
    // distinct declarations, so pointer provenance between them is not
    // guaranteed; integer math sidesteps that entirely.
    //
    // SAFETY: the linker script ALIGN(8)s the section start and brackets
    // exactly the KEEP'd `groundhog_rv32_entrypoints` input section, whose
    // sole writer is the #[contract] macro (each entry a `static Entrypoint`,
    // so the section is a well-formed array: size_of::<Entrypoint>() == 16
    // includes the tail padding, matching the section stride). An empty table
    // gives start == end -> len 0, valid for `from_raw_parts` with any
    // aligned non-null pointer.
    unsafe {
        let start = &raw const __groundhog_rv32_entrypoints_start;
        let end = &raw const __groundhog_rv32_entrypoints_end;
        let len = (end as usize - start as usize)
            / core::mem::size_of::<Entrypoint>();
        core::slice::from_raw_parts(start, len)
    }
}

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
    if let Some(wrapper) = lookup(entrypoints(), method_id) {
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
