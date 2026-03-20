use crate::xdr::*;
use core::any::Any;
use core::error::Error;
use core::fmt::{self, Debug, Display};

#[cfg(feature = "debugging")]
pub use with_strings::*;

#[cfg(not(feature = "debugging"))]
pub use without_strings::*;

// This is the type serialized by the `with_strings` implementation
#[cfg(any(test, feature = "debugging"))]
type StrOpt<'a> = Option<&'a str>;

#[cfg(any(test, feature = "debugging"))]
mod with_strings {
    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    pub struct FaultString(Option<heapless::String<500>>);

    impl WriteXDR for FaultString {
        fn write_xdr<F, W: Writer<F>>(&self, w: &mut W) -> WriteResult<()> {
            let str_opt: StrOpt = self.0.as_ref().map(|s| s.as_str());

            str_opt.write_xdr(w)
        }
    }

    impl<'r> ReadXDR<'r> for FaultString {
        fn read_xdr<R: Reader<'r>>(r: &mut R) -> ReadResult<Self> {
            let str_opt: StrOpt = read_xdr(r)?;

            if let Some(s) = str_opt {
                s.try_into()
                    .map(|s| Self(Some(s)))
                    .map_err(|_| ReadError::MaxLengthExceeded)
            } else {
                Ok(Self(None))
            }
        }
    }

    impl FaultString {
        pub fn as_str(&self) -> &str {
            self.0.as_ref().map(|s| s.as_str()).unwrap_or("<fault>")
        }

        pub fn empty() -> Self {
            Self(None)
        }

        pub fn via_display<V>(v: &V) -> Self
        where
            V: Display + ?Sized,
        {
            use core::fmt::Write;

            let mut s = heapless::String::new();
            write!(s, "{}", v).unwrap_or_else(|_| {
                s.clear();
                let _ = write!(s, "<too-big>");
            });

            Self(Some(s))
        }

        pub fn from_display(s: impl Display) -> Self {
            Self::via_display(&s)
        }

        pub fn from_any(a: &dyn Any) -> Self {
            if let Some(s) = a.downcast_ref::<&str>() {
                Self::from_display(s)
                /*
                } else if let Some(s) = a.downcast_ref::<&std::string::String>() {
                    Self::via_display(s.as_str())
                        */
            } else {
                Self::from_display("<unknown>")
            }
        }
    }
}

#[cfg(any(test, not(feature = "debugging")))]
#[allow(dead_code)]
mod without_strings {
    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    pub struct FaultString;

    impl WriteXDR for FaultString {
        #[inline(always)]
        fn write_xdr<F, W: Writer<F>>(&self, _w: &mut W) -> WriteResult<()> {
            Ok(())
        }
    }

    impl<'r> ReadXDR<'r> for FaultString {
        #[inline(always)]
        fn read_xdr<R: Reader<'r>>(_r: &mut R) -> ReadResult<Self> {
            Ok(Self)
        }
    }

    impl FaultString {
        pub fn as_str(&self) -> &str {
            "<fault>"
        }

        #[inline]
        pub fn empty() -> Self {
            Self
        }

        #[allow(unused)]
        pub fn via_display<V>(_v: &V) -> Self
        where
            V: ?Sized,
        {
            Self
        }

        pub fn from_display<S>(_s: S) -> Self {
            Self
        }

        #[allow(unused)]
        pub fn from_any(_a: &dyn Any) -> Self {
            Self
        }
    }
}

impl Error for FaultString {}

impl Display for FaultString {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[test]
fn with_strings_roundtrip() {
    let bytes = txs::encode(&with_strings::FaultString::empty()).unwrap();
    let s: with_strings::FaultString = txs::decode(bytes.as_ref()).unwrap();
    assert_eq!(s.as_str(), "<fault>");

    let bytes =
        txs::encode(&with_strings::FaultString::from_display("foo")).unwrap();
    let s: with_strings::FaultString = txs::decode(bytes.as_ref()).unwrap();
    assert_eq!(s.as_str(), "foo");
}

#[test]
fn without_strings_roundtrip() {
    let bytes = txs::encode(&without_strings::FaultString).unwrap();
    assert!(bytes.is_empty());
    let s: without_strings::FaultString = txs::decode(bytes.as_ref()).unwrap();
    assert_eq!(s.as_str(), "<fault>");
}
