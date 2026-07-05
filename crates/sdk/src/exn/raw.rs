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
//! catch semantics are identical in both builds, with no shared/occupancy
//! state — any number of `RawException`s may be live concurrently, the
//! same as under `alloc`. The cost is paid per live value (stack, not a
//! static), and the memcpy that fills it only runs when an undecodable
//! exception is actually caught.
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
        #[cfg(all(feature = "debugging", feature = "std"))]
        crate::contract_op::abort_breadcrumb("Re-throwing", "Location", self);

        crate::contract_op::contract_abort(self.wire)
    }

    /// Detach into an owned [`RawException`] (one copy). Any number may
    /// be live at once; without `alloc` each value carries its own fixed
    /// SCRATCH_MAX-capacity buffer.
    pub fn to_owned(&self) -> RawException {
        RawException::own_wire(self.wire, self.selector, self.payload_start)
    }
}

impl core::fmt::Debug for RawExceptionRef<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(
            f,
            "RawExceptionRef {{ selector: 0x{:016X}, payload: {} bytes",
            self.selector,
            self.payload().len(),
        )?;
        #[cfg(feature = "debugging")]
        write!(f, ", hex: {}", crate::hex::hex_encode(&self.payload()))?;
        write!(f, " }}")
    }
}

/// An exception caught and detached from the call scratch: the payload is
/// copied once, into storage owned by the value itself, so it survives
/// arbitrary later SDK operations (including re-use of the call scratch).
///
/// Under `alloc` the copy is an owned exact-size heap buffer. In the
/// no-alloc guest the copy is an inline fixed [`SCRATCH_MAX`]-capacity
/// buffer embedded in the value (the same buffer type backing the call
/// scratch), so `RawException` — and any error enum containing one — is
/// roughly 4 KiB. That size is a deliberate trade of stack for uniformity:
/// catch semantics are identical in both builds, with no shared/occupancy
/// state — any number of `RawException`s may be live concurrently, the same
/// as under `alloc`. The cost is paid per live value (stack, not a static),
/// and the memcpy that fills it only runs when an undecodable exception is
/// actually caught.
#[cfg(feature = "alloc")]
pub struct RawException {
    selector: u64,
    /// Full abort wire bytes: `[CXS(selector)][payload]` — kept verbatim so
    /// `re_throw` preserves multi-hop fidelity.
    wire: alloc::vec::Vec<u8>,
    payload_start: usize,
}

#[cfg(not(feature = "alloc"))]
pub struct RawException {
    selector: u64,
    payload_start: u32,
    /// Full abort wire bytes: `[CXS(selector)][payload]`, copied inline
    /// (fixed `SCRATCH_MAX` capacity) so the value owns its data outright —
    /// no shared slot, no occupancy tracking.
    wire: crate::call::Buf,
}

impl RawException {
    /// Copy `wire` (already parsed: `selector`, `payload_start`) out of the
    /// caller's borrow into owned storage.
    ///
    /// Called by [`RawExceptionRef::to_owned`].
    pub(crate) fn own_wire(
        wire: &[u8],
        selector: u64,
        payload_start: usize,
    ) -> Self {
        #[cfg(feature = "alloc")]
        {
            Self {
                selector,
                wire: wire.to_vec(),
                payload_start,
            }
        }
        #[cfg(not(feature = "alloc"))]
        {
            let mut buf = crate::call::Buf::new();
            if buf.extend_from_slice(wire).is_err() {
                // Cannot happen in practice: `wire` is abort bytes drawn
                // from the call scratch, which is the same `SCRATCH_MAX`
                // capacity as `buf`. If it somehow did, propagate the
                // original borrowed `wire` verbatim rather than catch a
                // truncated copy.
                crate::contract_op::contract_abort(wire);
            }
            Self {
                selector,
                payload_start: payload_start as u32,
                wire: buf,
            }
        }
    }

    pub fn selector(&self) -> u64 {
        self.selector
    }

    /// Run `f` on the payload bytes.
    pub fn with_payload<R>(&self, f: impl FnOnce(&[u8]) -> R) -> R {
        #[cfg(feature = "alloc")]
        {
            f(&self.wire[self.payload_start..])
        }
        #[cfg(not(feature = "alloc"))]
        {
            f(&self.wire.as_slice()[self.payload_start as usize..])
        }
    }

    pub fn try_as<T: Exn>(&self) -> Option<T> {
        if self.selector == T::SELECTOR {
            self.with_payload(|payload| {
                crate::call::contract_call_decode_exn_prefix(payload).ok()
            })
        } else {
            None
        }
    }

    /// Re-throw to our caller with the original wire bytes, verbatim.
    #[cfg_attr(feature = "debugging", track_caller)]
    pub fn re_throw(&self) -> ! {
        #[cfg(all(feature = "debugging", feature = "std"))]
        crate::contract_op::abort_breadcrumb("Re-throwing", "Location", self);

        #[cfg(feature = "alloc")]
        {
            crate::contract_op::contract_abort(&self.wire)
        }
        #[cfg(not(feature = "alloc"))]
        {
            crate::contract_op::contract_abort(self.wire.as_slice())
        }
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
        let (selector, payload_start) = parse_abort_prefix(bytes)?;
        Ok(Self {
            selector,
            wire: bytes.to_vec(),
            payload_start,
        })
    }
}

impl PartialEq for RawException {
    fn eq(&self, other: &Self) -> bool {
        self.selector == other.selector
            && self.with_payload(|a| other.with_payload(|b| a == b))
    }
}

impl core::fmt::Debug for RawException {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let payload_len = self.with_payload(|p| p.len());
        write!(
            f,
            "RawException {{ selector: 0x{:016X}, payload: {} bytes",
            self.selector, payload_len,
        )?;
        #[cfg(feature = "debugging")]
        self.with_payload(|p| {
            write!(f, ", hex: {}", crate::hex::hex_encode(&p))
        })?;
        write!(f, " }}")
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
        c.with_payload(|p| assert_eq!(p, b"hello"));
    }

    #[test]
    fn from_abort_buf_parses_wire_format() {
        // wire = [CXS(u64 selector)][payload]
        let mut wire = crate::internal::encode_xdr(&0xABCDu64).unwrap();
        wire.extend_from_slice(b"payload-bytes");
        let c = RawException::from_abort_buf(&wire).unwrap();
        assert_eq!(c.selector(), 0xABCD);
        c.with_payload(|p| assert_eq!(p, b"payload-bytes"));
    }

    #[test]
    fn holds_oversized_payload_without_truncation_under_alloc() {
        let big = [0xABu8; crate::internal::SCRATCH_MAX + 1];
        let c = RawException::new(0x1234, &big);
        c.with_payload(|p| assert_eq!(p, &big[..]));
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
