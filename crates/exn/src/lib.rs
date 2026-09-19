//! Contract exceptions, split out of the SDK (like `groundhog-event`) so
//! host-side tooling below the SDK can decode thrown exceptions: the [`Exn`]
//! trait and the two-tier raw exception.
//!
//! - [`RawExceptionRef`] is the transient, borrowed form. In the SDK it only
//!   exists inside the `on_exn` dispatch closure of `try_call_raw_with`; its
//!   payload is a plain slice borrowed straight from the call scratch, valid
//!   exactly for the closure's lifetime, so it can never be observed stale.
//! - [`RawException`] is the owned form, produced by
//!   [`RawExceptionRef::to_owned`] when a caller needs to hold onto an
//!   exception past the call (one copy).
//!
//! Under `alloc` the copy is an owned exact-size heap buffer. In the
//! no-alloc guest the copy is an inline array sized to the protocol's
//! return-buffer cap, so `RawException`, and any error enum containing one,
//! is roughly 1 KiB. That size is a deliberate trade of stack for
//! uniformity: catch semantics are identical in both builds. Each `RawException` owns
//! its buffer, so any number may be live concurrently, the same as under
//! `alloc`. The cost is paid per live value (stack, not a static), and the
//! memcpy that fills it only runs when an undecodable exception is actually
//! caught.
//!
//! Zero-copy operations ([`RawExceptionRef::try_as`],
//! [`RawExceptionRef::payload`], re-throwing) never allocate or copy; only
//! [`to_owned`](RawExceptionRef::to_owned) does, and at most once.
//! Re-throwing is a syscall, so it lives in the SDK as its `ReThrow` impls
//! for both types.

#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;

use core::fmt::Debug;
use xdrs::{ReadError, ReadXDR, ReadXDROwned, WriteXDR, cxs};

/// The protocol's cap on a frame's return/abort buffer. The SDK rejects a
/// longer host-reported buffer before parsing it, so this is exactly the
/// capacity the no-alloc owned copy needs.
#[cfg(any(not(feature = "alloc"), test))]
const WIRE_MAX: usize =
    groundhog_xdr::transaction::kMaxReturnBufferLen as usize;

/// A trait for exception types that can be serialized across contract calls.
///
/// Each implementing type gets a unique u64 selector derived from its
/// canonical type signature (via `#[exn]`). The selector is used
/// for dispatch when deserializing exceptions on the client side.
///
/// Throwing only needs `WriteXDR`, so decoding is not a supertrait; the
/// generic decode sites ([`RawExceptionRef::try_as`] /
/// [`RawException::try_as`]) require `ReadXDROwned` explicitly.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a contract exception",
    label = "not a contract exception",
    note = "annotate `{Self}` with `#[contract_exception]` (or `#[exn]` if \
            you handle your own derives); `throws = ...` in \
            `#[contract(...)]` requires it"
)]
pub trait Exn: Debug + PartialEq + WriteXDR {
    const SELECTOR: u64;
}

/// A reference throws as its referent: same selector, same wire encoding
/// (`WriteXDR for &T` writes `T`). Lets throw paths borrow an exception
/// instead of cloning it.
impl<T: Exn> Exn for &T {
    const SELECTOR: u64 = T::SELECTOR;
}

/// Exn impl for Infallible: this type can never be instantiated,
/// so its selector is 0 (a reserved/unused value).
impl Exn for core::convert::Infallible {
    const SELECTOR: u64 = 0;
}

/// A raw exception borrowed from an abort buffer, `[CXS(selector)][payload]`.
/// See the crate docs for how it relates to [`RawException`].
///
/// `Copy`: all three fields are, and the SDK's `MatchExn` hands out reborrows
/// of it. Covariance in `'s` makes a copy usable at any shorter lifetime.
#[derive(Clone, Copy)]
pub struct RawExceptionRef<'s> {
    selector: u64,
    /// Full verbatim abort wire bytes: `[CXS(selector)][payload]`.
    wire: &'s [u8],
    payload_start: usize,
}

impl<'s> RawExceptionRef<'s> {
    /// Fails only when the buffer does not start with a CXS-encoded `u64`:
    /// the frame aborted with something other than a contract exception.
    pub fn parse(wire: &'s [u8]) -> Result<Self, ReadError> {
        let mut reader = cxs::buf_reader::BufReader::new(wire);
        let selector: u64 = ReadXDR::read_xdr(&mut reader)?;
        Ok(Self {
            selector,
            wire,
            payload_start: wire.len() - reader.remaining_slice().len(),
        })
    }

    pub fn selector(&self) -> u64 {
        self.selector
    }

    /// The full abort wire bytes, selector prefix included: what a re-throw
    /// aborts with.
    pub fn wire(&self) -> &'s [u8] {
        self.wire
    }

    pub fn payload(&self) -> &'s [u8] {
        &self.wire[self.payload_start..]
    }

    /// Decode as `T` if the selector matches, `None` otherwise.
    ///
    /// `None` covers two different situations, a selector belonging to some
    /// other type, and this type's selector over a payload that will not
    /// decode. Use [`try_decode`](Self::try_decode) when they need telling
    /// apart.
    ///
    /// `T` must contain no auth-token handle (the SDK's `Signed`): abort
    /// frames carry none, so one would decode to the uninitialized
    /// `u32::MAX` sentinel, a value shaped like an auth token that faults on
    /// use. Nothing is checked here; spec canonicalization rejects such a
    /// payload when the contract is built.
    pub fn try_as<T: Exn + ReadXDROwned>(&self) -> Option<T> {
        // Defined in terms of `try_decode` so the two cannot drift on which
        // payloads they accept.
        self.try_decode().and_then(|decoded| decoded.ok())
    }

    /// [`try_as`](Self::try_as) with the decode failure kept separate from
    /// the selector mismatch:
    ///
    /// - `None`: the selector belongs to some other exception type.
    /// - `Some(Ok(t))`: the selector matched and the payload decoded.
    /// - `Some(Err(e))`: the selector matched, but the payload does not
    ///   decode as `T`.
    ///
    /// The third case is not only a forged or truncated payload. An
    /// exception's selector is `sha256(major_version :: crate :: type)` and
    /// says nothing about the payload's layout, so a callee that adds an
    /// enum variant (a semver-compatible change that leaves the selector
    /// untouched) throws something an older caller cannot decode. Since
    /// [`try_as`] reports that as `None`, indistinguishable from an
    /// unrelated error type, a caller that must not confuse the two should
    /// reach for this instead.
    ///
    /// Note that decoding reads a *prefix*: a payload carrying extra
    /// trailing bytes decodes as the shorter shape and is reported as
    /// `Some(Ok(..))`. Appending a field is therefore accepted silently
    /// rather than surfacing here.
    ///
    /// [`try_as`]: Self::try_as
    pub fn try_decode<T: Exn + ReadXDROwned>(
        &self,
    ) -> Option<Result<T, ReadError>> {
        if self.selector != T::SELECTOR {
            return None;
        }
        let mut reader = cxs::buf_reader::BufReader::new(self.payload());
        Some(ReadXDR::read_xdr(&mut reader))
    }

    /// Detach into an owned [`RawException`] (one copy). Any number may
    /// be live at once; without `alloc` each value carries its own inline
    /// buffer.
    ///
    /// Carries over the already-parsed `selector`/`payload_start` so the
    /// two stay consistent by construction.
    pub fn to_owned(&self) -> RawException {
        #[cfg(feature = "alloc")]
        let wire = self.wire.to_vec();
        #[cfg(not(feature = "alloc"))]
        let wire = {
            // The slice bound cannot trip: the SDK rejects a host-reported
            // abort buffer longer than `WIRE_MAX` before parsing it.
            let mut buf = [0u8; WIRE_MAX];
            buf[..self.wire.len()].copy_from_slice(self.wire);
            Wire {
                buf,
                len: self.wire.len(),
            }
        };
        RawException {
            selector: self.selector,
            wire,
            payload_start: self.payload_start,
        }
    }
}

impl core::fmt::Debug for RawExceptionRef<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        fmt_exn(f, "RawExceptionRef", self.selector, self.payload())
    }
}

#[cfg(feature = "alloc")]
type Wire = alloc::vec::Vec<u8>;
/// Inline owned wire storage for the no-alloc guest.
#[cfg(not(feature = "alloc"))]
struct Wire {
    buf: [u8; WIRE_MAX],
    len: usize,
}

#[cfg(not(feature = "alloc"))]
impl AsRef<[u8]> for Wire {
    fn as_ref(&self) -> &[u8] {
        &self.buf[..self.len]
    }
}

/// An exception caught and detached from the call scratch: the payload is
/// copied once, into storage owned by the value itself, so it survives
/// arbitrary later SDK operations (including re-use of the call scratch).
/// Without `alloc` that storage is an inline array at the protocol's
/// return-buffer cap, so the value (and any error enum holding it) is
/// roughly 1 KiB; under `alloc` it is an exact-size heap allocation.
///
/// Deliberately not encodable: there is no `WriteXDR` impl. Re-throwing (the
/// SDK's `ReThrow` impl) aborts with the verbatim wire bytes;
/// [`try_as`](Self::try_as) decodes to a typed exception, with
/// [`selector`](Self::selector) / [`payload`](Self::payload) for raw
/// inspection.
pub struct RawException {
    selector: u64,
    /// Full abort wire bytes: `[CXS(selector)][payload]`, kept verbatim so
    /// a re-throw preserves multi-hop fidelity.
    wire: Wire,
    payload_start: usize,
}

impl RawException {
    #[inline]
    pub fn selector(&self) -> u64 {
        self.selector
    }

    /// The full abort wire bytes, selector prefix included: what a re-throw
    /// aborts with.
    #[inline]
    pub fn wire(&self) -> &[u8] {
        <Wire as AsRef<[u8]>>::as_ref(&self.wire)
    }

    /// The exception payload (everything after the selector prefix).
    #[inline]
    pub fn payload(&self) -> &[u8] {
        &self.wire()[self.payload_start..]
    }

    /// Borrow this owned exception as a transient ref, so `try_as` and the
    /// SDK's `MatchExn` share their one implementation with
    /// [`RawExceptionRef`] instead of duplicating it.
    pub fn as_ref_exn(&self) -> RawExceptionRef<'_> {
        RawExceptionRef {
            selector: self.selector,
            wire: self.wire(),
            payload_start: self.payload_start,
        }
    }

    /// See [`RawExceptionRef::try_as`].
    pub fn try_as<T: Exn + ReadXDROwned>(&self) -> Option<T> {
        self.as_ref_exn().try_as()
    }

    /// See [`RawExceptionRef::try_decode`].
    pub fn try_decode<T: Exn + ReadXDROwned>(
        &self,
    ) -> Option<Result<T, ReadError>> {
        self.as_ref_exn().try_decode()
    }

    /// Build from a selector and payload. Test/mock construction.
    #[cfg(feature = "alloc")]
    pub fn new(selector: u64, payload: &[u8]) -> Self {
        let mut wire = cxs::encode(&selector)
            .expect("u64 CXS-encode cannot fail under alloc");
        let payload_start = wire.len();
        wire.extend_from_slice(payload);
        Self {
            selector,
            wire,
            payload_start,
        }
    }

    /// Parse an abort wire buffer (`[CXS(selector)][payload]`) into an owned
    /// exception. Host tests and mock plumbing.
    #[cfg(feature = "alloc")]
    pub fn from_abort_buf(bytes: &[u8]) -> Result<Self, ReadError> {
        Ok(RawExceptionRef::parse(bytes)?.to_owned())
    }
}

impl PartialEq for RawException {
    fn eq(&self, other: &Self) -> bool {
        self.selector == other.selector && self.payload() == other.payload()
    }
}

impl core::fmt::Debug for RawException {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        fmt_exn(f, "RawException", self.selector, self.payload())
    }
}

/// Shared `Debug` body for the two raw exception types: same shape and
/// fields, differing only in the type name and how the payload is reached.
fn fmt_exn(
    f: &mut core::fmt::Formatter,
    name: &str,
    selector: u64,
    payload: &[u8],
) -> core::fmt::Result {
    write!(
        f,
        "{name} {{ selector: 0x{selector:016X}, payload: {} bytes",
        payload.len(),
    )?;
    #[cfg(not(target_arch = "riscv32"))]
    {
        write!(f, ", hex: ")?;
        for b in payload {
            write!(f, "{b:02x}")?;
        }
    }
    write!(f, " }}")
}

#[cfg(all(test, feature = "alloc"))]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    #[derive(Debug, PartialEq, xdrs::WriteXDR, xdrs::ReadXDR)]
    struct TestExn(u32);

    impl Exn for TestExn {
        const SELECTOR: u64 = 0xABCD;
    }

    fn wire(selector: u64, payload: &[u8]) -> Vec<u8> {
        let mut wire = cxs::encode(&selector).unwrap();
        wire.extend_from_slice(payload);
        wire
    }

    #[test]
    fn parse_separates_selector_and_payload() {
        let wire = wire(0xABCD, b"payload-bytes");
        let exn = RawExceptionRef::parse(&wire).unwrap();
        assert_eq!(exn.selector(), 0xABCD);
        assert_eq!(exn.payload(), b"payload-bytes");
        assert_eq!(exn.wire(), &wire[..]);
    }

    #[test]
    fn parse_rejects_garbage() {
        // Empty bytes cannot possibly contain a CXS-encoded u64 selector.
        assert!(RawExceptionRef::parse(&[]).is_err());
    }

    #[test]
    fn try_decode_distinguishes_selector_from_payload_failures() {
        let payload = cxs::encode(&7u32).unwrap();
        let ok = wire(TestExn::SELECTOR, &payload);
        let ok = RawExceptionRef::parse(&ok).unwrap();
        assert!(matches!(ok.try_decode::<TestExn>(), Some(Ok(TestExn(7)))));
        assert_eq!(ok.try_as::<TestExn>(), Some(TestExn(7)));

        let other = wire(TestExn::SELECTOR + 1, &payload);
        let other = RawExceptionRef::parse(&other).unwrap();
        assert!(other.try_decode::<TestExn>().is_none());
        assert_eq!(other.try_as::<TestExn>(), None);

        let truncated = wire(TestExn::SELECTOR, &[]);
        let truncated = RawExceptionRef::parse(&truncated).unwrap();
        assert!(matches!(truncated.try_decode::<TestExn>(), Some(Err(_))));
        assert_eq!(truncated.try_as::<TestExn>(), None);
    }

    #[test]
    fn to_owned_keeps_wire_selector_and_payload() {
        let wire = wire(TestExn::SELECTOR, b"hello");
        let owned = RawExceptionRef::parse(&wire).unwrap().to_owned();
        assert_eq!(owned.selector(), TestExn::SELECTOR);
        assert_eq!(owned.payload(), b"hello");
        assert_eq!(owned.wire(), &wire[..]);
        assert_eq!(owned.as_ref_exn().payload(), b"hello");
    }

    #[test]
    fn new_roundtrips_selector_and_payload() {
        let c = RawException::new(0x1234, b"hello");
        assert_eq!(c.selector(), 0x1234);
        assert_eq!(c.payload(), b"hello");
    }

    #[test]
    fn from_abort_buf_parses_wire_format() {
        let wire = wire(0xABCD, b"payload-bytes");
        let c = RawException::from_abort_buf(&wire).unwrap();
        assert_eq!(c.selector(), 0xABCD);
        assert_eq!(c.payload(), b"payload-bytes");
        assert!(RawException::from_abort_buf(&[]).is_err());
    }

    #[test]
    fn holds_oversized_payload_without_truncation_under_alloc() {
        let big = [0xABu8; WIRE_MAX + 1];
        let c = RawException::new(0x1234, &big);
        assert_eq!(c.payload(), &big[..]);
    }

    #[test]
    fn eq_compares_selector_and_payload_bytes() {
        assert_eq!(RawException::new(1, b"x"), RawException::new(1, b"x"));
        assert_ne!(RawException::new(1, b"x"), RawException::new(2, b"x"));
        assert_ne!(RawException::new(1, b"x"), RawException::new(1, b"y"));
    }
}
