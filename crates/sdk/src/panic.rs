// The rv32 `#[panic_handler]` and `#[global_allocator]` live in
// `groundhog-rv32-runtime`, which `sdk` force-links on rv32 targets (see
// `lib.rs`). Natively, the standard library supplies the panic handler.
//
// This module retains only the `Panic` *type* below, which the exception
// machinery (`exn`, `throw`, `re_throw`, …) uses to carry panic info under the
// `debugging` feature — it is unrelated to the panic handler.

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
    #[derive(Clone, Debug, PartialEq, ReadXDR, WriteXDR)]
    pub struct Panic;

    impl Error for Panic {}

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
