//! System functions: trap and panic

#[cfg(target_arch = "wasm32")]
#[inline(always)]
pub fn trap() -> ! {
    core::arch::wasm32::unreachable()
}

#[cfg(all(not(target_arch = "wasm32"), not(feature = "mock")))]
#[inline(always)]
pub fn trap() -> ! {
    panic!("unreachable");
}

#[cfg(feature = "mock")]
mod test_rt {
    use native_exec::invoke_error::deterministic_error;
    use std::boxed::Box;
    use std::panic::panic_any;

    pub fn trap() -> ! {
        panic_any(Box::new(deterministic_error()))
    }
}

#[cfg(feature = "mock")]
#[allow(dead_code)]
#[allow(unused)]
pub use test_rt::trap;

// When natively-compiled contracts panic, we'll see a message printed from the panic handler that
// comes with the standard library.
//
// For wasm contracts, we can instead use the `log()` syscall to report the panic. Enable feature
// "panic_logging" for this.
//
// A contract compiled with the rustc flag `-Cpanic=immediate_abort` won't ever call
// `handle_panic`. (That setting is useful because it results in panic strings not being included
// in the binary.) The build scripts may be configured to instead choose `panic=abort`, which will
// enable the logging below.
//
#[cfg(not(feature = "std"))]
#[panic_handler]
fn handle_panic(#[allow(unused)] info: &core::panic::PanicInfo) -> ! {
    #[cfg(feature = "panic_logging")]
    {
        use crate::log;
        use core::fmt::Write;
        use heapless::String;

        let mut msg: String<150> = String::new();

        write!(msg, "Contract panic: {}", info.message()).unwrap_or_else(
            |_| {
                // Failure only happens in case the static `msg` buffer is too small,
                // which may be discovered after a partial write.  We could just log
                // whatever has been written, but that's not usually useful, so instead:

                let _ =
                    log(b"Contract panic: <couldn't format panic message>");

                trap();
            },
        );

        let _ = log(msg.as_str().as_bytes());
    }

    trap();
}
