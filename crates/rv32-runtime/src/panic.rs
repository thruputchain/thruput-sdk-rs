//! `#[panic_handler]` for rv32 contracts.
//!
//! Release contract profiles use `-Cpanic=immediate-abort`, which strips the
//! panic path entirely. Where the handler is reached we issue `ebreak`, which
//! the Thruput VM traps as `Trap::kBreakpoint` and the runtime maps to a
//! deterministic failure.
//!
// `ebreak` (0x00100073) fails the call immediately, rather than spinning
// until the gas budget runs out.

use core::arch::asm;

#[panic_handler]
fn handle_panic(_info: &core::panic::PanicInfo) -> ! {
    // EBREAK traps the VM (kBreakpoint -> deterministic failure) and never
    // returns; `noreturn` satisfies the `!` return type.
    unsafe {
        asm!("ebreak", options(noreturn));
    }
}
