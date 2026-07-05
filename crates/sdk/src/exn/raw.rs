//! The two-tier exception design:
//!
//! - [`RawException`] is the transient, borrowed form. It only exists inside
//!   the `on_exn` dispatch closure of
//!   [`try_call_raw_with`](crate::call::try_call_raw_with) — its payload is
//!   a plain slice borrowed straight from the call scratch, valid exactly
//!   for the closure's lifetime, so it can never be observed stale.
//! - [`CaughtException`](crate::exn::CaughtException) is the owned form,
//!   produced by [`RawException::caught`] when a caller needs to hold onto
//!   an exception past the call (one copy).
//!
//! Zero-copy operations — [`RawException::try_as`], [`RawException::payload`],
//! [`RawException::re_throw`] — never allocate or copy; only [`caught`](RawException::caught)
//! does, and at most once.

use crate::call::contract_call_decode_exn_prefix;
use crate::exn::{
    CallingError, CaughtException, Exn, FaultString, LibraryExn,
};
use crate::xdr::*;

/// A raw exception received from a contract call, borrowed from the call
/// scratch. Only exists inside the `on_exn` dispatch closure of
/// [`try_call_raw_with`](crate::call::try_call_raw_with) — the borrow ends
/// with the closure, so it can never be observed stale. To keep it past the
/// call, [`caught`](Self::caught) copies it into an owned
/// [`CaughtException`].
pub struct RawException<'s> {
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

impl<'s> RawException<'s> {
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

    /// Detach into an owned [`CaughtException`] (one copy). Any number may
    /// be live at once; without `alloc` each value carries its own fixed
    /// SCRATCH_MAX-capacity buffer.
    pub fn caught(&self) -> CaughtException {
        CaughtException::catch_wire(
            self.wire,
            self.selector,
            self.payload_start,
        )
    }
}

impl core::fmt::Debug for RawException<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(
            f,
            "RawException {{ selector: 0x{:016X}, payload: {} bytes",
            self.selector,
            self.payload().len(),
        )?;
        #[cfg(feature = "debugging")]
        write!(f, ", hex: {}", crate::hex::hex_encode(&self.payload()))?;
        write!(f, " }}")
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

        let exn = RawException::parse(&wire).unwrap();

        assert_eq!(exn.selector(), 0xABCD);
        assert_eq!(exn.payload(), b"payload-bytes");
    }

    #[test]
    fn parse_rejects_garbage() {
        // Empty bytes cannot possibly contain a CXS-encoded u64 selector.
        assert!(RawException::parse(&[]).is_err());
    }
}
