//! rv32 `_start`: the guest dispatch entry.
//!
//! The hogd VM enters every call at PC 0 (`_start`) with the invoked
//! `method_id` split across `a0` (low 32) and `a1` (high 32). `_start` is a
//! naked shim: it sets `sp` and tail-calls `rv32_dispatch` WITHOUT touching
//! `a0`/`a1`, so the RISC-V C ABI reassembles them into the `u64` argument for
//! free. The dispatcher scans the dispatch table populated by the
//! `#[contract]` macro (one `Entrypoint` record per method, emitted into the
//! `thruput_rv32_entrypoints` section) and calls the matching wrapper.

use crate::dispatch::{Entrypoint, lookup};
use core::arch::naked_asm;

// Dispatch-table bracket symbols. The linker script
// (link/thruput-rv32.ld) defines both around the KEEP'd
// `thruput_rv32_entrypoints` input section. Declared as `Entrypoint` so
// `&raw const` yields correctly-typed table pointers; the "values" are never
// read directly, only their addresses.
unsafe extern "C" {
    static __thruput_rv32_entrypoints_start: Entrypoint;
    static __thruput_rv32_entrypoints_end: Entrypoint;
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
    // exactly the KEEP'd `thruput_rv32_entrypoints` input section, whose
    // sole writer is the #[contract] macro (each entry a `static Entrypoint`,
    // so the section is a well-formed array: size_of::<Entrypoint>() == 16
    // includes the tail padding, matching the section stride). An empty table
    // gives start == end -> len 0, valid for `from_raw_parts` with any
    // aligned non-null pointer. The pointer is non-null: the section lives in
    // `.rodata`, which the linker script places at `__stack_top`
    // (= `ORIGIN(DATA) + __stack_size`, ASSERTed >= 16), so both bracket
    // symbols are always > 0.
    unsafe {
        let start = &raw const __thruput_rv32_entrypoints_start;
        let end = &raw const __thruput_rv32_entrypoints_end;
        let len = (end as usize - start as usize)
            / core::mem::size_of::<Entrypoint>();
        core::slice::from_raw_parts(start, len)
    }
}

// The dispatcher's terminal arm, defined by `thruput-sdk` (see
// `thruput_sdk::__rv32`): abort the frame with the typed
// `LibraryExn::UnknownMethod`, which the caller can decode by selector. It
// replaced an `ebreak` here — a `Trap::kBreakpoint` the caller could not tell
// apart from an OOB access or a bad instruction — for 20 bytes of .text, since
// `library_abort` is already linked into every contract.
//
// This crate cannot build the exception itself: it is dependency-free by
// design and `thruput-sdk` depends on *it*, so the two meet at this symbol.
// Nothing links this crate without `thruput-sdk` (`thruput-sdk` force-links it
// on rv32), so the definition is always there.
unsafe extern "C" {
    #[link_name = "__thruput_unknown_method"]
    safe fn unknown_method_exn() -> !;
}

/// C-ABI entry called by the naked `_start`. `method_id` arrives reassembled
/// from the seeded `a0`/`a1`.
///
/// Falls through to the unknown-method abort when no table entry matches, and
/// also if a wrapper returns — it shouldn't (wrappers diverge via the Return
/// syscall), so that case is unreachable in practice and merely reports itself
/// as an unknown method if a bug ever reaches it.
extern "C" fn rv32_dispatch(method_id: u64) -> ! {
    if let Some(wrapper) = lookup(entrypoints(), method_id) {
        wrapper();
    }
    unknown_method_exn()
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
    // gp is loaded first so every subsequent static access is eligible for
    // the linker's gp relaxation (see link/thruput-rv32.ld).
    // `naked` guarantees no prologue clobbers a0/a1.
    naked_asm!(
        // Load gp with the linker-script relaxation anchor BEFORE anything
        // can touch a static. The norelax bracket keeps this la itself from
        // being relaxed into a gp-relative move — gp isn't set yet.
        ".option push",
        ".option norelax",
        "la gp, __global_pointer$",
        ".option pop",
        "la sp, __stack_top",
        "tail {dispatch}",
        dispatch = sym rv32_dispatch,
    );
}
