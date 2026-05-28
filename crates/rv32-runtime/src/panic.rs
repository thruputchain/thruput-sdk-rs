//! `#[panic_handler]` for rv32 contracts.
//!
//! WASM contracts use `-Cpanic=immediate_abort` to strip the panic path
//! entirely. For rv32 we fall into a tight infinite loop; gas exhaustion
//! produces the OOG outcome.
//!
//! Notes:
//!   - We cannot use `ebreak`: hogd's Program::validate accepts ECALL
//!     as the only SYSTEM-opcode instruction. EBREAK (0x00100073) is
//!     rejected as kUnsupportedSystemInstruction and would invalidate
//!     the whole artifact at parse time. Until the VM accepts EBREAK
//!     as a deterministic trap, an infinite loop is the right shape.
//!   - TODO(rv32): route panic info to the host `Abort` syscall so the
//!     host surfaces a structured panic record (panic_code, optional
//!     message) to the runtime trace. That needs a `groundhog-xdr`
//!     dep, gated on the `panic_abort_syscall` cargo feature.

use core::arch::asm;

#[panic_handler]
fn handle_panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
        unsafe {
            asm!("", options(nomem, nostack));
        }
    }
}
