//! The message carried alongside a contract fault.
//!
//! `FaultString` encodes to **zero bytes on every target and every feature
//! combination**. The message it carries is host-side diagnostic state, not
//! wire data: it is written where the fault is raised and read by the native
//! test harness, and it never reaches a serializer. That is deliberate —
//! `FaultString` appears inside [`LibraryExn`](crate::exn::LibraryExn), whose
//! encoding must not vary with how the crate was built, or a contract's abort
//! payload would differ between a host test and the artifact that ships.
//!
//! On `riscv32` the message is absent entirely: the field, the formatting, and
//! the buffer all compile out, so a guest pays nothing for it.

use crate::xdr::*;
use core::any::Any;
use core::error::Error;
use core::fmt::{self, Debug, Display};

/// Capacity of the host-side message. Host-only: a guest has no field to size.
#[cfg(not(target_arch = "riscv32"))]
const CAP: usize = 500;

/// A fault's diagnostic message.
///
/// Carries a human-readable description of a fault on the host and nothing at
/// all on a guest. Encodes to zero bytes regardless — see the module docs.
/// Two `FaultString`s are always equal: the message is diagnostic, so it is
/// not part of the value's identity, and comparisons behave the same on host
/// and guest.
#[derive(Clone, Default)]
pub struct FaultString {
    #[cfg(not(target_arch = "riscv32"))]
    msg: Option<heapless::String<CAP>>,
}

impl FaultString {
    /// The message, or `"<fault>"` when none was recorded (always, on a
    /// guest).
    pub fn as_str(&self) -> &str {
        #[cfg(not(target_arch = "riscv32"))]
        {
            self.msg.as_ref().map(|s| s.as_str()).unwrap_or("<fault>")
        }
        #[cfg(target_arch = "riscv32")]
        {
            "<fault>"
        }
    }

    /// A fault with no message.
    #[inline]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Record `v`'s `Display` form as the message, truncating to `CAP`.
    #[cfg(not(target_arch = "riscv32"))]
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

        Self { msg: Some(s) }
    }

    /// Discard `v`: a guest records no message.
    ///
    /// Deliberately unbounded rather than `V: Display`. Several callers pass
    /// an error whose `Display` is itself only compiled in some
    /// configurations, so requiring the bound here would fail the guest
    /// build for types that never get formatted on a guest anyway.
    #[cfg(target_arch = "riscv32")]
    pub fn via_display<V: ?Sized>(_v: &V) -> Self {
        Self {}
    }

    /// [`via_display`](Self::via_display) for a value passed by value.
    #[cfg(not(target_arch = "riscv32"))]
    pub fn from_display(s: impl Display) -> Self {
        Self::via_display(&s)
    }

    /// [`via_display`](Self::via_display) for a value passed by value.
    #[cfg(target_arch = "riscv32")]
    pub fn from_display<S>(_s: S) -> Self {
        Self {}
    }

    /// Recover a message from a panic payload, when it is a string.
    #[cfg_attr(target_arch = "riscv32", allow(unused))]
    pub fn from_any(a: &dyn Any) -> Self {
        if let Some(s) = a.downcast_ref::<&str>() {
            Self::from_display(s)
        } else {
            Self::from_display("<unknown>")
        }
    }
}

// Hand-written rather than derived: the message is host-only state, so
// deriving would make equality (and `Debug`'s shape) differ by target.
impl PartialEq for FaultString {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl Eq for FaultString {}

impl Debug for FaultString {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("FaultString").field(&self.as_str()).finish()
    }
}

impl Error for FaultString {}

impl Display for FaultString {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// Wire form: empty, on every target. Hand-written because the struct has a
// target-gated field that must never reach the encoding -- see module docs.
impl WriteXDR for FaultString {
    fn write_xdr<F, W: xdrs::Writer<F>>(
        &self,
        _w: &mut W,
    ) -> xdrs::WriteResult<()> {
        Ok(())
    }

    unsafe fn write_xdr_unchecked<F, W: xdrs::WriterUnchecked<F>>(
        &self,
        _w: &mut W,
    ) {
    }
}

impl<'r> ReadXDR<'r> for FaultString {
    fn read_xdr<R: xdrs::Reader<'r>>(_r: &mut R) -> xdrs::ReadResult<Self> {
        Ok(Self::empty())
    }
}

impl xdrs::cxs::ConstMaxSize for FaultString {
    const MAX_SIZE: Option<usize> = Some(0);
}

impl xdrs::txs::ConstMaxSize for FaultString {
    const MAX_SIZE: Option<usize> = Some(0);
}

// A guest carries no message: no field, no buffer, no formatting. Pinned
// here so a `cfg` slip that let the host-side field through would fail the
// guest build rather than quietly costing every contract 500-odd bytes of
// stack per live fault value.
#[cfg(target_arch = "riscv32")]
const _: () = assert!(core::mem::size_of::<FaultString>() == 0);

// The property the rest of the exception machinery depends on: a
// `FaultString` contributes nothing to any enclosing type's size bound, so
// `LibraryExn`'s wire size (and `ABORT_MAX`) cannot vary with the message or
// with how the crate was built.
const _: () = assert!(matches!(
    <FaultString as xdrs::cxs::ConstMaxSize>::MAX_SIZE,
    Some(0)
));

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

#[cfg(test)]
mod tests {
    use super::*;

    /// The wire form is empty whatever the message -- this is the property
    /// that keeps a contract's abort payload identical across build configs.
    #[test]
    fn encodes_to_nothing_whatever_the_message() {
        for fs in [
            FaultString::empty(),
            FaultString::from_display("boom"),
            FaultString::from_display(
                core::str::from_utf8(&[b'x'; 500]).unwrap(),
            ),
        ] {
            assert!(txs::encode(&fs).unwrap().is_empty());
        }
    }

    #[test]
    fn decodes_from_nothing() {
        let s: FaultString = txs::decode(&[]).unwrap();
        assert_eq!(s.as_str(), "<fault>");
    }

    /// The message is readable in-process, which is the only place it is
    /// ever meant to be read.
    #[test]
    fn message_is_readable_host_side() {
        assert_eq!(FaultString::from_display("boom").as_str(), "boom");
        assert_eq!(FaultString::empty().as_str(), "<fault>");
    }

    /// An oversized message degrades rather than panicking or truncating
    /// into something misleading.
    #[test]
    fn oversized_message_is_marked() {
        let long = core::str::from_utf8(&[b'x'; CAP + 1]).unwrap();
        assert_eq!(FaultString::from_display(long).as_str(), "<too-big>");
    }
}
