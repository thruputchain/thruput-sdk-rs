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
use core::fmt::{self, Debug};

// Host-only, like everything that reads or renders a fault's message.
#[cfg(not(target_arch = "riscv32"))]
use core::any::Any;
#[cfg(not(target_arch = "riscv32"))]
use core::error::Error;
#[cfg(not(target_arch = "riscv32"))]
use core::fmt::Display;

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
    /// The message, or `"<fault>"` when none was recorded.
    ///
    /// Host-only: a guest records no message, so there is nothing for this
    /// to return and it does not exist there. Reading a fault's text is a
    /// diagnostic act, and diagnostics are a host concern.
    #[cfg(not(target_arch = "riscv32"))]
    pub fn as_str(&self) -> &str {
        self.msg.as_ref().map(|s| s.as_str()).unwrap_or("<fault>")
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
    ///
    /// Host-only: a guest neither unwinds nor records a message. Nothing in
    /// the SDK calls this yet — it is the intended entry point for the
    /// native harness to hand a caught callee panic to
    /// [`RawCallError::Fault`](crate::call::RawCallError::Fault), which is
    /// where a fault's message would have to come from. Nothing on the call
    /// boundary carries one.
    #[cfg(not(target_arch = "riscv32"))]
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

// `Debug` is the one formatting impl a guest needs: `LibraryExn` derives it,
// and `contract_abort_encoded` takes `T: Debug`. The guest arm prints no
// message because it has none to print.
#[cfg(not(target_arch = "riscv32"))]
impl Debug for FaultString {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("FaultString").field(&self.as_str()).finish()
    }
}

// Writes nothing rather than the type's name: a guest has no message, and a
// name would be a string literal in every contract that instantiates this
// impl, for a value that is always empty there. The impl exists only because
// the bound demands it -- `LibraryExn` derives `Debug` and the throw helpers
// take `T: Debug` -- not because a guest has anything to print.
#[cfg(target_arch = "riscv32")]
impl Debug for FaultString {
    fn fmt(&self, _f: &mut fmt::Formatter) -> fmt::Result {
        Ok(())
    }
}

#[cfg(not(target_arch = "riscv32"))]
impl Error for FaultString {}

#[cfg(not(target_arch = "riscv32"))]
impl Display for FaultString {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// A guest has no message, so this renders nothing -- deliberately not even a
// placeholder, which would put a string in every contract for a value that is
// always empty there. It exists at all because generated client code can
// format a `FaultString` with `{}`.
#[cfg(target_arch = "riscv32")]
impl core::fmt::Display for FaultString {
    fn fmt(&self, _f: &mut fmt::Formatter) -> fmt::Result {
        Ok(())
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

/// Where a fault happened, as `panic!` recorded it.
///
/// Host-only: the location comes from the panic hook, which a guest has no
/// equivalent of — its `#[panic_handler]` ignores `PanicInfo` entirely.
#[cfg(not(target_arch = "riscv32"))]
#[derive(Clone, PartialEq, Eq)]
pub struct FaultLocation {
    file: heapless::String<FILE_CAP>,
    line: u32,
    column: u32,
}

/// Capacity of the recorded file path. Paths longer than this keep their
/// tail, which is the part that identifies the file.
#[cfg(not(target_arch = "riscv32"))]
const FILE_CAP: usize = 200;

#[cfg(not(target_arch = "riscv32"))]
impl FaultLocation {
    /// Record `file:line:column`, keeping the tail of an oversized path.
    pub fn new(file: &str, line: u32, column: u32) -> Self {
        let tail = match file.len().checked_sub(FILE_CAP) {
            None => file,
            // A path longer than the cap keeps its end: the leading
            // directories are the interchangeable part.
            Some(excess) => {
                let mut start = excess;
                while start < file.len() && !file.is_char_boundary(start) {
                    start += 1;
                }
                &file[start..]
            }
        };
        let mut stored = heapless::String::new();
        // Cannot fail: `tail` is at most FILE_CAP bytes by construction.
        let _ = stored.push_str(tail);
        Self {
            file: stored,
            line,
            column,
        }
    }

    pub fn file(&self) -> &str {
        &self.file
    }

    pub fn line(&self) -> u32 {
        self.line
    }

    pub fn column(&self) -> u32 {
        self.column
    }
}

#[cfg(not(target_arch = "riscv32"))]
impl Display for FaultLocation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}:{}:{}", self.file, self.line, self.column)
    }
}

#[cfg(not(target_arch = "riscv32"))]
impl Debug for FaultLocation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{self}")
    }
}

/// What is known about the panic that faulted a frame: its message, and
/// where it happened.
///
/// Both halves are host-side diagnostic state, kept out of the wire form for
/// the same reason [`FaultString`] is — see the module docs. On `riscv32`
/// this is a zero-sized type: the message field compiles out with
/// `FaultString` and the location has nowhere to come from, since a guest's
/// `#[panic_handler]` ignores `PanicInfo` and issues `ebreak`.
///
/// Two `FaultInfo`s are always equal, following [`FaultString`]: the contents
/// are diagnostic, not part of the value's identity, so comparisons behave
/// the same on host and guest.
#[derive(Clone, Default)]
pub struct FaultInfo {
    message: FaultString,
    #[cfg(not(target_arch = "riscv32"))]
    location: Option<FaultLocation>,
}

impl FaultInfo {
    /// A fault with nothing recorded.
    #[inline]
    pub fn empty() -> Self {
        Self::default()
    }

    /// A fault carrying only a message.
    pub fn from_message(message: FaultString) -> Self {
        Self {
            message,
            #[cfg(not(target_arch = "riscv32"))]
            location: None,
        }
    }

    /// The message, or `"<fault>"` when none was recorded.
    #[cfg(not(target_arch = "riscv32"))]
    pub fn as_str(&self) -> &str {
        self.message.as_str()
    }

    /// The recorded message.
    pub fn message(&self) -> &FaultString {
        &self.message
    }

    /// Where the panic happened, when the harness recorded it.
    ///
    /// Absent when nothing was recorded for the fault at all — a
    /// `raw_Invoke`ed callee's panic is caught inside the runtime's own
    /// frame, which reports a status code and nothing else — and when a
    /// panic reached the catch site with no matching hook record, which
    /// leaves a message with no location beside it.
    #[cfg(not(target_arch = "riscv32"))]
    pub fn location(&self) -> Option<&FaultLocation> {
        self.location.as_ref()
    }
}

// Hand-written for the same reason as `FaultString`'s: the contents are
// host-only state, so deriving would make equality differ by target.
impl PartialEq for FaultInfo {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl Eq for FaultInfo {}

/// Renders as `message at file:line:column`, dropping either half that was
/// not recorded. With neither, this is `FaultString`'s `"<fault>"`.
#[cfg(not(target_arch = "riscv32"))]
impl Display for FaultInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.location {
            Some(location) => write!(f, "{} at {location}", self.message),
            None => write!(f, "{}", self.message),
        }
    }
}

#[cfg(not(target_arch = "riscv32"))]
impl Debug for FaultInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("FaultInfo")
            .field("message", &self.message)
            .field("location", &self.location)
            .finish()
    }
}

// Writes nothing, for the reason `FaultString`'s guest `Debug` does. This
// impl exists only to satisfy `RawCallError`'s `#[derive(Debug)]`; removing
// it fails the guest build, so it cannot simply be dropped the way the guest
// `Display` was.
#[cfg(target_arch = "riscv32")]
impl Debug for FaultInfo {
    fn fmt(&self, _f: &mut fmt::Formatter) -> fmt::Result {
        Ok(())
    }
}

#[cfg(not(target_arch = "riscv32"))]
impl Error for FaultInfo {}

// The harness's record is the only source of a populated `FaultInfo`, and
// `PanicRecord` keeps the location as flat fields rather than a struct of its
// own: it is the capture layer, where the hook hands over an unbounded path,
// while `FaultLocation` is the bounded, no-alloc form contract-facing code
// sees. One type cannot serve both — `sdk` depends on `native-exec` and never
// the reverse, and that dep is optional and host-only, so this conversion
// exists only under `dev`.
#[cfg(all(not(target_arch = "riscv32"), feature = "dev"))]
impl From<&groundhog_native_exec::invoke_error::PanicRecord> for FaultInfo {
    fn from(
        record: &groundhog_native_exec::invoke_error::PanicRecord,
    ) -> Self {
        // Destructured so a field added to the record fails to compile here
        // rather than being silently dropped.
        let groundhog_native_exec::invoke_error::PanicRecord {
            message,
            file,
            line,
            column,
        } = record;
        Self {
            message: message
                .as_deref()
                .map_or_else(FaultString::empty, FaultString::from_display),
            location: file
                .as_deref()
                .map(|f| FaultLocation::new(f, *line, *column)),
        }
    }
}

// A guest carries neither half, so the whole struct vanishes. Pinned here so
// a `cfg` slip that let the host-side state through would fail the guest
// build rather than quietly costing every contract stack per live fault.
#[cfg(target_arch = "riscv32")]
const _: () = assert!(core::mem::size_of::<FaultInfo>() == 0);
