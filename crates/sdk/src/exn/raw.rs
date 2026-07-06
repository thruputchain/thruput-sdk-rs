//! The two-tier exception design:
//!
//! - [`RawExceptionRef`] is the transient, borrowed form. It only exists
//!   inside the `on_exn` dispatch closure of
//!   [`try_call_raw_with`](crate::call::try_call_raw_with) — its payload is
//!   a plain slice borrowed straight from the call scratch, valid exactly
//!   for the closure's lifetime, so it can never be observed stale.
//! - [`RawException`] is the owned form, produced by
//!   [`RawExceptionRef::to_owned`] when a caller needs to hold onto an
//!   exception past the call (one copy).
//!
//! Under `alloc` the copy is an owned exact-size heap buffer. In the
//! no-alloc guest the copy is an inline fixed [`SCRATCH_MAX`]-capacity
//! buffer embedded in the value (the same buffer type backing the call
//! scratch), so `RawException` — and any error enum containing one — is
//! roughly 4 KiB. That size is a deliberate trade of stack for uniformity:
//! catch semantics are identical in both builds — each `RawException` owns
//! its buffer, so any number may be live concurrently, the same as under
//! `alloc`. The cost is paid per live value (stack, not a static), and the
//! memcpy that fills it only runs when an undecodable exception is actually
//! caught.
//!
//! Zero-copy operations — [`RawExceptionRef::try_as`], [`RawExceptionRef::payload`],
//! [`RawExceptionRef::re_throw`] — never allocate or copy; only
//! [`to_owned`](RawExceptionRef::to_owned) does, and at most once.
//!
//! [`SCRATCH_MAX`]: crate::internal::SCRATCH_MAX

use crate::call::contract_call_decode_exn_prefix;
use crate::exn::{CallingError, Exn, FaultString, LibraryExn};
use crate::xdr::*;

/// The protocol caps a child frame's return/abort payload at
/// `kMaxReturnBufferLen`; the no-alloc call scratch (and `RawException`'s
/// inline no-alloc copy) is a fixed `SCRATCH_MAX`-capacity buffer. This is
/// what makes the `extend_from_slice` overflow branch in
/// [`own_wire`](RawException::own_wire) below unreachable in practice:
/// `wire` can never exceed `SCRATCH_MAX` bytes.
const _: () = assert!(
    crate::types::transaction::kMaxReturnBufferLen as usize
        <= crate::internal::SCRATCH_MAX
);

/// A raw exception received from a contract call, borrowed from the call
/// scratch. Only exists inside the `on_exn` dispatch closure of
/// [`try_call_raw_with`](crate::call::try_call_raw_with) — the borrow ends
/// with the closure, so it can never be observed stale. To keep it past the
/// call, [`to_owned`](Self::to_owned) copies it into an owned
/// [`RawException`].
pub struct RawExceptionRef<'s> {
    selector: u64,
    /// Full verbatim abort wire bytes: `[CXS(selector)][payload]`.
    wire: &'s [u8],
    payload_start: usize,
}

/// Parse `[CXS(selector)][payload bytes...]`, returning the selector and the
/// payload's start offset.
pub(super) fn parse_abort_prefix(
    bytes: &[u8],
) -> Result<(u64, usize), LibraryExn> {
    let mut reader = cxs::buf_reader::BufReader::new(bytes);
    let selector: u64 = ReadXDR::read_xdr(&mut reader).map_err(|e| {
        LibraryExn::Calling(CallingError::DecodingException(
            FaultString::from_display(e),
        ))
    })?;
    Ok((selector, bytes.len() - reader.remaining_slice().len()))
}

impl<'s> RawExceptionRef<'s> {
    pub(crate) fn parse(wire: &'s [u8]) -> Result<Self, LibraryExn> {
        let (selector, payload_start) = parse_abort_prefix(wire)?;
        Ok(Self {
            selector,
            wire,
            payload_start,
        })
    }

    pub fn selector(&self) -> u64 {
        self.selector
    }

    pub fn payload(&self) -> &'s [u8] {
        &self.wire[self.payload_start..]
    }

    pub fn try_as<T: Exn>(&self) -> Option<T> {
        if self.selector == T::SELECTOR {
            contract_call_decode_exn_prefix(self.payload()).ok()
        } else {
            None
        }
    }

    /// Re-throw to our caller: abort with the original wire bytes, verbatim.
    /// Zero-copy — this is what plain (non-`try_`) generated methods do.
    #[inline]
    #[cfg_attr(feature = "debugging", track_caller)]
    pub fn re_throw(&self) -> ! {
        self.re_throw_shown(self)
    }

    /// Shared re-throw body: aborts with `self`'s wire bytes, but prints
    /// `shown`'s `Debug` form in the breadcrumb rather than `self`'s. Lets
    /// [`RawException::re_throw`] delegate here (via [`as_ref_exn`](RawException::as_ref_exn))
    /// while still breadcrumbing as "RawException {...}" rather than
    /// "RawExceptionRef {...}" — the type the caller actually held.
    #[inline]
    #[cfg_attr(feature = "debugging", track_caller)]
    #[cfg_attr(
        not(all(feature = "debugging", feature = "std")),
        allow(unused_variables)
    )]
    fn re_throw_shown(&self, shown: &dyn core::fmt::Debug) -> ! {
        #[cfg(all(feature = "debugging", feature = "std"))]
        crate::contract_op::abort_breadcrumb("Re-throwing", "Location", shown);

        crate::contract_op::contract_abort(self.wire)
    }

    /// Detach into an owned [`RawException`] (one copy). Any number may
    /// be live at once; without `alloc` each value carries its own fixed
    /// SCRATCH_MAX-capacity buffer.
    pub fn to_owned(&self) -> RawException {
        RawException::own_wire(self)
    }
}

impl core::fmt::Debug for RawExceptionRef<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        fmt_exn(f, "RawExceptionRef", self.selector, self.payload())
    }
}

/// Shared `Debug` body for [`RawExceptionRef`] and [`RawException`]: same
/// shape and fields, differing only in the type name and how the payload
/// bytes are reached.
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
    #[cfg(feature = "debugging")]
    write!(f, ", hex: {}", crate::hex::hex_encode(&payload))?;
    write!(f, " }}")
}

/// An exception caught and detached from the call scratch: the payload is
/// copied once, into storage owned by the value itself, so it survives
/// arbitrary later SDK operations (including re-use of the call scratch).
/// Without `alloc` that storage is a fixed `SCRATCH_MAX`-capacity inline
/// buffer, so the value (and any error enum holding it) is roughly 4 KiB;
/// under `alloc` it is an exact-size heap allocation.
///
/// Deliberately not encodable: there is no `WriteXDR` impl.
/// [`re_throw`](Self::re_throw) aborts with the verbatim wire bytes;
/// [`try_as`](Self::try_as) decodes to a typed exception, with
/// [`selector`](Self::selector) / [`payload`](Self::payload) for raw
/// inspection.
pub struct RawException {
    selector: u64,
    /// Full abort wire bytes: `[CXS(selector)][payload]` — kept verbatim so
    /// `re_throw` preserves multi-hop fidelity.
    wire: crate::call::Buf,
    payload_start: usize,
}

impl RawException {
    /// Copy `exn`'s wire bytes out of the caller's borrow into owned
    /// storage, carrying over its already-parsed `selector`/`payload_start`
    /// so the two stay consistent by construction.
    ///
    /// Called by [`RawExceptionRef::to_owned`].
    pub(crate) fn own_wire(exn: &RawExceptionRef<'_>) -> Self {
        let selector = exn.selector;
        let payload_start = exn.payload_start;
        let wire = exn.wire;
        #[cfg(feature = "alloc")]
        let wire = wire.to_vec();
        #[cfg(not(feature = "alloc"))]
        let wire = {
            let mut buf = crate::call::Buf::new();
            if buf.extend_from_slice(wire).is_err() {
                // Cannot happen in practice: `wire` is abort bytes drawn
                // from the call scratch, which is the same `SCRATCH_MAX`
                // capacity as `buf` (see the const assert above). If it
                // somehow did, propagate the original borrowed `wire`
                // verbatim rather than catch a truncated copy.
                crate::contract_op::contract_abort(wire);
            }
            buf
        };
        Self {
            selector,
            wire,
            payload_start,
        }
    }

    #[inline]
    pub fn selector(&self) -> u64 {
        self.selector
    }

    /// The exception payload (everything after the selector prefix).
    #[inline]
    pub fn payload(&self) -> &[u8] {
        &self.wire.as_slice()[self.payload_start..]
    }

    /// Borrow this owned exception as a transient ref, so `try_as`/`re_throw`
    /// share their one implementation with [`RawExceptionRef`] instead of
    /// duplicating it.
    fn as_ref_exn(&self) -> RawExceptionRef<'_> {
        RawExceptionRef {
            selector: self.selector,
            wire: self.wire.as_slice(),
            payload_start: self.payload_start,
        }
    }

    pub fn try_as<T: Exn>(&self) -> Option<T> {
        self.as_ref_exn().try_as()
    }

    /// Re-throw to our caller with the original wire bytes, verbatim.
    #[inline]
    #[cfg_attr(feature = "debugging", track_caller)]
    pub fn re_throw(&self) -> ! {
        // The breadcrumb print (and the abort itself) live in
        // `RawExceptionRef::re_throw_shown`; every function in the chain is
        // `track_caller`, so the reported location is still the original
        // call site here, not this delegation. Passing `self` (rather than
        // `self.as_ref_exn().re_throw()`) keeps the breadcrumb's `Debug`
        // output labeled "RawException {...}", matching the type this
        // caller actually holds.
        self.as_ref_exn().re_throw_shown(self)
    }

    /// Build from a selector and payload. Test/mock construction.
    #[cfg(feature = "alloc")]
    pub fn new(selector: u64, payload: &[u8]) -> Self {
        let mut wire = crate::internal::encode_xdr(&selector)
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
    pub fn from_abort_buf(bytes: &[u8]) -> Result<Self, LibraryExn> {
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

impl crate::re_throw::ReThrow for RawException {
    #[cfg_attr(feature = "debugging", track_caller)]
    fn re_throw(self) -> ! {
        RawException::re_throw(&self)
    }
}

#[cfg(all(test, feature = "alloc"))]
mod tests {
    use super::*;

    #[test]
    fn parse_extracts_selector_and_payload() {
        // wire = [CXS(u64 selector)][payload]
        let mut wire = crate::internal::encode_xdr(&0xABCDu64).unwrap();
        wire.extend_from_slice(b"payload-bytes");

        let exn = RawExceptionRef::parse(&wire).unwrap();

        assert_eq!(exn.selector(), 0xABCD);
        assert_eq!(exn.payload(), b"payload-bytes");
    }

    #[test]
    fn parse_rejects_garbage() {
        // Empty bytes cannot possibly contain a CXS-encoded u64 selector.
        assert!(RawExceptionRef::parse(&[]).is_err());
    }

    #[test]
    fn new_roundtrips_selector_and_payload() {
        let c = RawException::new(0x1234, b"hello");
        assert_eq!(c.selector(), 0x1234);
        assert_eq!(c.payload(), b"hello");
    }

    #[test]
    fn from_abort_buf_parses_wire_format() {
        // wire = [CXS(u64 selector)][payload]
        let mut wire = crate::internal::encode_xdr(&0xABCDu64).unwrap();
        wire.extend_from_slice(b"payload-bytes");
        let c = RawException::from_abort_buf(&wire).unwrap();
        assert_eq!(c.selector(), 0xABCD);
        assert_eq!(c.payload(), b"payload-bytes");
    }

    #[test]
    fn holds_oversized_payload_without_truncation_under_alloc() {
        let big = [0xABu8; crate::internal::SCRATCH_MAX + 1];
        let c = RawException::new(0x1234, &big);
        assert_eq!(c.payload(), &big[..]);
    }

    #[test]
    fn eq_compares_selector_and_payload_bytes() {
        assert_eq!(RawException::new(1, b"x"), RawException::new(1, b"x"));
        assert_ne!(RawException::new(1, b"x"), RawException::new(2, b"x"));
        assert_ne!(RawException::new(1, b"x"), RawException::new(1, b"y"));
    }

    #[test]
    fn try_as_decodes_matching_selector_and_payload() {
        let payload =
            crate::internal::encode_xdr(&LibraryExn::NonNegativeIntOverflow)
                .unwrap();
        let c = RawException::new(LibraryExn::SELECTOR, &payload);

        assert_eq!(
            c.try_as::<LibraryExn>(),
            Some(LibraryExn::NonNegativeIntOverflow)
        );
    }

    #[test]
    fn try_as_returns_none_for_mismatched_selector() {
        let payload =
            crate::internal::encode_xdr(&LibraryExn::NonNegativeIntOverflow)
                .unwrap();
        // A valid payload for LibraryExn, but tagged with the wrong selector.
        let c = RawException::new(!LibraryExn::SELECTOR, &payload);

        assert_eq!(c.try_as::<LibraryExn>(), None);
    }

    #[test]
    fn try_as_returns_none_for_undecodable_payload() {
        let payload =
            crate::internal::encode_xdr(&LibraryExn::NonNegativeIntOverflow)
                .unwrap();
        // Truncate so the discriminant itself can't be read back.
        let truncated = &payload[..payload.len().saturating_sub(1)];
        let c = RawException::new(LibraryExn::SELECTOR, truncated);

        assert_eq!(c.try_as::<LibraryExn>(), None);
    }
}
