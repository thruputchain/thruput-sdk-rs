// When natively-compiled contracts panic, we'll see a message printed from the
// panic handler that comes with the standard library.
//
// For wasm contracts under test, we can instead use the `log()` syscall to
// report the panic. Enable feature "panic_logging" for this.
//
// A contract compiled with the rustc flag `-Cpanic=immediate_abort` won't ever
// call `handle_panic`. (That setting is useful because it results in panic
// strings not being included in the binary.) The build scripts may be
// configured to instead choose `panic=abort`, which will enable the logging
// below.
//
#[cfg(target_arch = "wasm32")]
#[panic_handler]
fn handle_panic(info: &core::panic::PanicInfo) -> ! {
    use abort::*;

    #[cfg(feature = "panic_logging")]
    panic_logging(info);

    if abort_handler_is_present() {
        abort(&Panic::from_info(info));
    } else {
        let _ = crate::log(b"Panicked but there is no abort handler!");
        crate::trap();
    }
}

mod panic_repr {
    use core::error::Error;
    use core::fmt::{self, Display, Write};
    use heapless::String;
    use xdrs::*;

    /// A representation of a panic condition, for sending to `abort`
    ///
    #[derive(Clone, Debug, PartialEq, ReadXDR, WriteXDR)]
    pub struct Panic {
        pub(crate) msg: String<300>,
        pub(crate) write_failed: bool,
    }

    impl Error for Panic {}

    impl Panic {
        pub fn from_info(info: &core::panic::PanicInfo) -> Self {
            let mut write_failed = false;
            let mut msg = String::new();
            if write!(&mut msg, "{}", info.message()).is_err() {
                write_failed = true;
            } else if let Some(loc) = info.location()
                && write!(&mut msg, " at {}", loc).is_err()
            {
                write_failed = true;
            }

            Self { msg, write_failed }
        }
    }

    impl Display for Panic {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(
                f,
                "panic: {}{}",
                self.msg.as_str(),
                if self.write_failed {
                    " (may be truncated)"
                } else {
                    ""
                },
            )
        }
    }
}
pub use panic_repr::*;

#[cfg(target_arch = "wasm32")]
#[inline(always)]
fn panic_logging(info: &core::panic::PanicInfo) {
    use crate::log;
    use core::fmt::Write;
    use heapless::String;

    let mut msg: String<150> = String::new();

    write!(msg, "Contract panic: {}", info.message()).unwrap_or_else(|_| {
        // Failure only happens in case the static `msg` buffer is too
        // small, which may be discovered after a partial write.  We
        // could just log whatever has been written, but that's not
        // usually useful, so instead:

        let _ = log(b"Contract panic: <couldn't format panic message>");

        crate::trap();
    });

    let _ = log(msg.as_str().as_bytes());
}
