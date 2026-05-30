//! Default `_start`.
//!
//! The hogd VM picks the entry PC from the artifact dispatch table, not
//! from the ELF entry header. So `_start` is only invoked when the
//! dispatch routes method ID 0 here — which is unusual; contracts should
//! emit an explicit `gh_method_0` symbol via `entry_point!`.
//!
//! When that doesn't happen, falling through to this stub is a bug:
//! issue a deterministic trap so it's visible during contract bring-up.

use core::arch::asm;

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text._start")]
pub extern "C" fn _start() -> ! {
    // Initialize sp to the top of guest memory rounded down to 16 bytes.
    // The hogd VM also does this in Machine::reset(), so this matches.
    // We don't strictly need it since the VM sets it, but a real ELF
    // entry point should be safe to call from a stale stack.
    unsafe {
        asm!(
            // __stack_top is provided by the linker script.
            // For now reuse __heap_end which is the top of RAM.
            "la sp, __heap_end",
            "andi sp, sp, -16",
            options(nostack),
        );
    }

    // Default dispatch landed at _start instead of a real method entry
    // point — an unrecoverable contract bug. Issue `ebreak`: the hogd VM
    // now traps it as Trap::kBreakpoint (deterministic failure), so the
    // bug surfaces immediately during bring-up. (Before the VM accepted
    // EBREAK we fell into a tight loop and relied on OOG.)
    unsafe {
        asm!("ebreak", options(noreturn));
    }
}
