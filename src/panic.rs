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

    abort(&Panic::from_info(info));
}

#[cfg(any(test, feature = "debugging"))]
mod with_message {
    use core::error::Error;
    use core::fmt::{self, Display, Write};
    use core::panic::Location;
    use heapless::String;
    use xdrs::*;

    /// A representation of a panic condition, for sending to `abort`.
    ///
    /// When `debugging` is enabled, captures the panic message and
    /// location in a heapless string buffer.
    ///
    #[derive(Clone, Debug, PartialEq, ReadXDR, WriteXDR)]
    pub struct Panic {
        pub(crate) msg: String<300>,
        pub(crate) write_failed: bool,
    }

    impl Error for Panic {}

    impl Panic {
        pub fn from_info(info: &core::panic::PanicInfo) -> Self {
            Self::from_hook_info(
                info.message().as_str().unwrap_or("<has args>"),
                info.location(),
            )
        }

        pub fn from_hook_info(
            message: &str,
            location: Option<&Location>,
        ) -> Self {
            let mut write_failed = false;

            let mut msg = String::new();

            if write!(&mut msg, "{}", message).is_err() {
                write_failed = true;
            } else if let Some(loc) = location
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

#[cfg(any(test, not(feature = "debugging")))]
#[allow(dead_code)]
mod without_message {
    use core::error::Error;
    use core::fmt::{self, Display};
    use xdrs::*;

    /// A representation of a panic condition, for sending to `abort`.
    ///
    /// Without `debugging`, this is a zero-size type that serializes
    /// to nothing — the heapless::String<300> and formatting code are
    /// not compiled in.
    ///
    #[derive(Clone, Debug, PartialEq)]
    pub struct Panic;

    impl Error for Panic {}

    impl Panic {
        #[cfg(target_arch = "wasm32")]
        pub fn from_info(_info: &core::panic::PanicInfo) -> Self {
            Self
        }
    }

    impl WriteXDR for Panic {
        #[inline(always)]
        fn write_xdr<F, W: Writer<F>>(&self, _w: &mut W) -> WriteResult<()> {
            Ok(())
        }
    }

    impl<'r> ReadXDR<'r> for Panic {
        #[inline(always)]
        fn read_xdr<R: Reader<'r>>(_r: &mut R) -> ReadResult<Self> {
            Ok(Self)
        }
    }

    impl Display for Panic {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("<panic>")
        }
    }
}

#[cfg(feature = "debugging")]
pub use with_message::*;

#[cfg(not(feature = "debugging"))]
pub use without_message::*;

#[cfg(all(target_arch = "wasm32", feature = "panic_logging"))]
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

#[cfg(test)]
mod tests {
    use crate::xdr::*;

    #[test]
    fn without_message_roundtrip() {
        // Without debugging, Panic is a unit struct that serializes to nothing
        use super::without_message::Panic;
        let bytes = txs::encode(&Panic).unwrap();
        assert!(bytes.is_empty());
        let decoded: Panic = txs::decode(bytes.as_ref()).unwrap();
        assert_eq!(decoded, Panic);
    }

    #[test]
    fn with_message_roundtrip() {
        use super::with_message::Panic;
        let p = Panic::from_hook_info("test panic", None);
        let bytes = txs::encode(&p).unwrap();
        let decoded: Panic = txs::decode(bytes.as_ref()).unwrap();
        assert_eq!(decoded, p);
    }
}
