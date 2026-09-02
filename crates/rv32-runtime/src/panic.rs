//! `#[panic_handler]` for rv32 contracts.
//!
//! Release contract profiles use `-Cpanic=immediate-abort`, which strips the
//! panic path entirely. Where the handler is reached we issue `ebreak`, which
//! the hogd VM traps as `Trap::kBreakpoint` and the runtime maps to a
//! deterministic failure.
//!
// Notes:
//   - hogd now accepts `ebreak` (0x00100073) as a deterministic-trap
//     SYSTEM instruction (alongside ECALL). Earlier the VM rejected it
//     at parse time, so we fell into a tight infinite loop and relied on
//     gas exhaustion (OOG); `ebreak` is strictly better — it fails
//     immediately instead of burning the whole gas budget.
//   - TODO(rv32): route panic info to the host `Abort` syscall so the
//     host surfaces a structured panic record (panic_code, optional
//     message) to the runtime trace. That needs a `groundhog-xdr`
//     dep, gated on the `panic_abort_syscall` cargo feature.

use core::arch::asm;

#[panic_handler]
fn handle_panic(_info: &core::panic::PanicInfo) -> ! {
    // EBREAK traps the VM (kBreakpoint -> deterministic failure) and never
    // returns; `noreturn` satisfies the `!` return type.
    unsafe {
        asm!("ebreak", options(noreturn));
    }
}
