//! `#[panic_handler]` for wasm contracts.
//!
//! A contract compiled with `-Cpanic=immediate-abort` never calls this (that
//! setting drops the panic path entirely, and is what release builds use to
//! keep panic strings out of the binary). When a build instead uses
//! `panic=abort`, this handler runs: it traps the module, optionally logging
//! the panic message first when the `panic_logging` feature is enabled.

#[panic_handler]
fn handle_panic(info: &core::panic::PanicInfo) -> ! {
    #[cfg(feature = "panic_logging")]
    log_panic(info);

    #[cfg(not(feature = "panic_logging"))]
    let _ = info;

    trap()
}

/// WASM trap: execute an illegal instruction.
#[cfg(target_arch = "wasm32")]
#[inline(always)]
fn trap() -> ! {
    core::arch::wasm32::unreachable()
}

/// Doc-only stub: `doc` builds compile this module on the host, where the
/// wasm intrinsic is unavailable. Never built into a real artifact.
#[cfg(all(doc, not(target_arch = "wasm32")))]
#[inline(always)]
fn trap() -> ! {
    loop {}
}

/// Best-effort: format the panic message and emit it via the host
/// `ConsoleLog` syscall. Runs inside a panic, so any error is ignored —
/// `handle_panic` traps immediately afterward regardless.
#[cfg(feature = "panic_logging")]
#[inline(always)]
fn log_panic(info: &core::panic::PanicInfo) {
    use core::fmt::Write;
    use groundhog_xdr::syscalls::raw_ConsoleLog;
    use heapless::String;

    let mut msg: String<150> = String::new();

    if write!(msg, "Contract panic: {}", info.message()).is_err() {
        // The static buffer was too small; log a fixed notice instead.
        let _ =
            raw_ConsoleLog(b"Contract panic: <couldn't format panic message>");
        return;
    }

    let _ = raw_ConsoleLog(msg.as_str().as_bytes());
}
