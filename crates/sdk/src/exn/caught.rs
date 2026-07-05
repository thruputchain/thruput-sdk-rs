//! An exception caught and detached from the call scratch: the payload is
//! copied once, into storage owned by the value itself, so it survives
//! arbitrary later SDK operations (including re-use of the call scratch).
//!
//! Under `alloc` the copy is an owned exact-size heap buffer. In the
//! no-alloc guest the copy is an inline fixed [`SCRATCH_MAX`]-capacity
//! buffer embedded in the value (the same buffer type backing the call
//! scratch), so `CaughtException` — and any error enum containing one — is
//! roughly 4 KiB. That size is a deliberate trade of stack for uniformity:
//! catch semantics are identical in both builds, with no shared/occupancy
//! state — any number of `CaughtException`s may be live concurrently, the
//! same as under `alloc`. The cost is paid per live value (stack, not a
//! static), and the memcpy that fills it only runs when an undecodable
//! exception is actually caught.
//!
//! [`SCRATCH_MAX`]: crate::internal::SCRATCH_MAX

use super::Exn;
#[cfg(feature = "alloc")]
use super::LibraryExn;

#[cfg(feature = "alloc")]
pub struct CaughtException {
    selector: u64,
    /// Full abort wire bytes: `[CXS(selector)][payload]` — kept verbatim so
    /// `re_throw` preserves multi-hop fidelity.
    wire: alloc::vec::Vec<u8>,
    payload_start: usize,
}

#[cfg(not(feature = "alloc"))]
pub struct CaughtException {
    selector: u64,
    payload_start: u32,
    /// Full abort wire bytes: `[CXS(selector)][payload]`, copied inline
    /// (fixed `SCRATCH_MAX` capacity) so the value owns its data outright —
    /// no shared slot, no occupancy tracking.
    wire: crate::call::Buf,
}

impl CaughtException {
    /// Copy `wire` (already parsed: `selector`, `payload_start`) out of the
    /// caller's borrow into owned storage.
    ///
    /// Called by [`RawException::caught`](crate::exn::RawException::caught).
    pub(crate) fn catch_wire(
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
    pub fn re_throw(&self) -> ! {
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
        let (selector, payload_start) = super::raw::parse_abort_prefix(bytes)?;
        Ok(Self {
            selector,
            wire: bytes.to_vec(),
            payload_start,
        })
    }
}

impl PartialEq for CaughtException {
    fn eq(&self, other: &Self) -> bool {
        self.selector == other.selector
            && self.with_payload(|a| other.with_payload(|b| a == b))
    }
}

impl core::fmt::Debug for CaughtException {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let payload_len = self.with_payload(|p| p.len());
        write!(
            f,
            "CaughtException {{ selector: 0x{:016X}, payload: {} bytes }}",
            self.selector, payload_len,
        )
    }
}

impl crate::re_throw::ReThrow for CaughtException {
    fn re_throw(self) -> ! {
        CaughtException::re_throw(&self)
    }
}

#[cfg(all(test, feature = "alloc"))]
mod tests {
    use super::*;

    #[test]
    fn new_roundtrips_selector_and_payload() {
        let c = CaughtException::new(0x1234, b"hello");
        assert_eq!(c.selector(), 0x1234);
        c.with_payload(|p| assert_eq!(p, b"hello"));
    }

    #[test]
    fn from_abort_buf_parses_wire_format() {
        // wire = [CXS(u64 selector)][payload]
        let mut wire = crate::internal::encode_xdr(&0xABCDu64).unwrap();
        wire.extend_from_slice(b"payload-bytes");
        let c = CaughtException::from_abort_buf(&wire).unwrap();
        assert_eq!(c.selector(), 0xABCD);
        c.with_payload(|p| assert_eq!(p, b"payload-bytes"));
    }

    #[test]
    fn holds_oversized_payload_without_truncation_under_alloc() {
        let big = [0xABu8; crate::internal::SCRATCH_MAX + 1];
        let c = CaughtException::new(0x1234, &big);
        c.with_payload(|p| assert_eq!(p, &big[..]));
    }

    #[test]
    fn eq_compares_selector_and_payload_bytes() {
        assert_eq!(
            CaughtException::new(1, b"x"),
            CaughtException::new(1, b"x")
        );
        assert_ne!(
            CaughtException::new(1, b"x"),
            CaughtException::new(2, b"x")
        );
        assert_ne!(
            CaughtException::new(1, b"x"),
            CaughtException::new(1, b"y")
        );
    }
}
