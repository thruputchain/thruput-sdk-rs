use crate::xdr::*;
use core::any::Any;
use core::error::Error;
use core::fmt::{self, Debug, Display};

#[cfg(feature = "debugging")]
pub use with_strings::*;

#[cfg(not(feature = "debugging"))]
pub use without_strings::*;

#[cfg(any(test, feature = "debugging"))]
mod with_strings {
    use super::*;

    /// Capacity of the carried message.
    const CAP: usize = 500;

    // Everything about the wire is derived from the field type
    // (`Option<String<CAP>>`): encode, decode, unchecked encode, and
    // the size bounds. `heapless::String` encodes exactly like `&str`.
    #[derive(Debug, Clone, PartialEq, ReadXDR, WriteXDR, ConstXDRSize)]
    pub struct FaultString(Option<heapless::String<CAP>>);

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

        #[cfg_attr(test, allow(dead_code))]
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

    impl crate::__contract_spec::ContractTypeName for FaultString {
        const NAME: &'static str = "FaultString";
    }

    impl crate::__contract_spec::ContractTypeSpec for FaultString {
        const TYPE_DEF: crate::__contract_spec::ContractTypeDefView<'static> =
            crate::__contract_spec::ContractTypeDefView::Struct(
                crate::__contract_spec::ContractStructDefView {
                    name: <Self as crate::__contract_spec::ContractTypeName>::NAME,
                    fields: &[],
                },
            );
    }

    impl crate::__contract_spec::ContractTypeRefSpec for FaultString {
        const INFO: crate::__contract_spec::ContractTypeInfo<'static> =
            crate::__contract_spec::ContractTypeInfo {
                ty: crate::__contract_spec::ContractTypeView::Named(
                    <Self as crate::__contract_spec::ContractTypeName>::NAME,
                ),
            };

        const __GROUNDHOG_SPEC_ANCHOR_ROOT: fn() -> usize =
            Self::__groundhog_spec_anchor;

        #[cfg_attr(target_arch = "riscv32", inline(never))]
        #[cfg_attr(
            target_arch = "riscv32",
            unsafe(link_section = "groundhogspecanchortextv0")
        )]
        fn __groundhog_spec_anchor() -> usize {
            0
        }
    }

    #[doc(hidden)]
    #[allow(non_upper_case_globals)]
    pub const __groundhog_spec_fragment_FaultString:
        crate::__contract_spec::SpecFragmentView<'static> =
        crate::__contract_spec::SpecFragmentView::TypeDef(
            <FaultString as crate::__contract_spec::ContractTypeSpec>::TYPE_DEF,
        );
}

#[cfg(any(test, not(feature = "debugging")))]
#[allow(dead_code)]
mod without_strings {
    use super::*;

    #[derive(
        Debug,
        Clone,
        PartialEq,
        ReadXDR,
        WriteXDR,
        crate::ContractTypeSpec,
        ConstXDRSize,
    )]
    pub struct FaultString;

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

    impl crate::__contract_spec::ContractTypeName for FaultString {
        const NAME: &'static str = "FaultString";
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
