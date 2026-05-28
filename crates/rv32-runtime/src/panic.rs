//! `#[panic_handler]` for rv32 contracts.
//!
//! WASM contracts use `-Cpanic=immediate_abort` to strip the panic path
//! entirely, or fall through to the `log()` syscall when
//! `feature = "panic_logging"` is on. For rv32 we mirror the simpler
//! strategy: trap via `ebreak`, which the VM converts to a deterministic
//! trap (`Trap::kEbreak`).
//!
//! TODO(rv32): route panic info to the host `Abort` syscall so the host
//! surfaces a structured panic record (`panic_code`, optional message)
//! to the runtime trace. That needs a `groundhog-xdr` dep, gated on the
//! `panic_abort_syscall` cargo feature.

use core::arch::asm;

#[panic_handler]
fn handle_panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
        unsafe {
            asm!("ebreak", options(nomem, nostack));
        }
    }
}
