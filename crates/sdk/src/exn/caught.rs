//! An exception caught and detached from the call scratch: the payload is
//! copied once so the value survives arbitrary later SDK operations.
//!
//! No-alloc: the copy lives in a dedicated per-instance static slot
//! (`EXN_SLOT`), guarded by an occupancy flag released on `Drop`. While a
//! `CaughtException` is alive nothing may overwrite the slot, so payload
//! access is infallible. At most one may be live at a time: catching a
//! second one propagates it (re-aborts verbatim) instead of clobbering.
//! Under `alloc` the copy is an owned exact-size buffer (many may be live;
//! the native mock nests frames).

use super::Exn;
#[cfg(feature = "alloc")]
use super::LibraryExn;

#[cfg(not(feature = "alloc"))]
use crate::cell::{ContractLocalRef, ContractRefCell};

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
    wire_end: u32,
}

#[cfg(not(feature = "alloc"))]
struct ExnSlot {
    occupied: bool,
    buf: crate::call::Buf,
}

#[cfg(not(feature = "alloc"))]
crate::contract_local! {
    static EXN_SLOT: ContractRefCell<ExnSlot> = const {
        ContractRefCell::new(ExnSlot { occupied: false, buf: crate::call::Buf::new() })
    };
}

impl CaughtException {
    /// Copy `wire` (already parsed: `selector`, `payload_start`) out of the
    /// caller's borrow into owned storage. No-alloc: if the slot is
    /// unavailable (another `CaughtException` is live, or we are inside its
    /// `with_payload`), aborts with `wire` verbatim — propagate over clobber.
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
            EXN_SLOT.with_ref(|cell| match cell.try_borrow_mut() {
                Ok(mut slot) if !slot.occupied => {
                    slot.buf.clear();
                    if slot.buf.extend_from_slice(wire).is_err() {
                        // Wire bytes exceed the slot (cannot happen: both are
                        // SCRATCH_MAX-sized) — propagate rather than truncate.
                        crate::contract_op::contract_abort(wire);
                    }
                    slot.occupied = true;
                    Self {
                        selector,
                        payload_start: payload_start as u32,
                        wire_end: wire.len() as u32,
                    }
                }
                _ => crate::contract_op::contract_abort(wire),
            })
        }
    }

    pub fn selector(&self) -> u64 {
        self.selector
    }

    /// Run `f` on the payload bytes. Infallible: while `self` is alive the
    /// slot cannot be overwritten.
    pub fn with_payload<R>(&self, f: impl FnOnce(&[u8]) -> R) -> R {
        #[cfg(feature = "alloc")]
        {
            f(&self.wire[self.payload_start..])
        }
        #[cfg(not(feature = "alloc"))]
        {
            EXN_SLOT.with_ref(|cell| {
                let slot = cell.borrow();
                f(&slot.buf.as_slice()
                    [self.payload_start as usize..self.wire_end as usize])
            })
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
            EXN_SLOT.with_ref(|cell| {
                let slot = cell.borrow();
                crate::contract_op::contract_abort(
                    &slot.buf.as_slice()[..self.wire_end as usize],
                )
            })
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

#[cfg(not(feature = "alloc"))]
impl Drop for CaughtException {
    fn drop(&mut self) {
        EXN_SLOT.with_ref(|cell| {
            if let Ok(mut slot) = cell.try_borrow_mut() {
                slot.occupied = false;
            }
        });
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
