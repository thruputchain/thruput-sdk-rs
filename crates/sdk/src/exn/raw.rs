use crate::call::contract_call_decode_exn_prefix;
use crate::exn::{CallingError, Exn, FaultString, LibraryExn};
use crate::xdr::*;
use core::fmt::{self, Debug};

/// A raw exception received from a contract call.
///
/// Contains the selector and remaining payload bytes. Use `try_as::<T>()`
/// to attempt decoding as a known exception type.
pub struct RawException {
    selector: u64,
    payload: crate::call::Buf,
}

impl RawException {
    /// Build a `RawException` from a selector and payload bytes.
    ///
    /// With `alloc` this is infallible. Without `alloc` the payload is held in
    /// a fixed [`SCRATCH_MAX`](crate::internal::SCRATCH_MAX)-byte buffer; a
    /// payload larger than that returns `Err` (a typed
    /// [`CallingError::DecodingException`]) rather than being silently
    /// truncated, which would yield a mis-decoded exception. Use
    /// [`or_re_throw`](crate::re_throw::OrReThrow::or_re_throw) to turn the
    /// error into an abort at the call site.
    pub fn new(selector: u64, payload: &[u8]) -> Result<Self, LibraryExn> {
        let buf = crate::call::buf_from_slice(payload).map_err(|()| {
            LibraryExn::Calling(CallingError::DecodingException(
                FaultString::from_display(format_args!(
                    "exception payload ({} bytes) exceeds the no-alloc \
                     buffer capacity ({})",
                    payload.len(),
                    crate::internal::SCRATCH_MAX,
                )),
            ))
        })?;
        Ok(Self {
            selector,
            payload: buf,
        })
    }

    /// Decode a RawException from an abort buffer.
    /// The buffer is CXS-encoded: [u64 selector][payload bytes...].
    pub fn from_abort_buf(bytes: &[u8]) -> Result<Self, LibraryExn> {
        let mut reader = cxs::buf_reader::BufReader::new(bytes);
        let selector: u64 = ReadXDR::read_xdr(&mut reader).map_err(|e| {
            LibraryExn::Calling(CallingError::DecodingException(
                FaultString::from_display(e),
            ))
        })?;
        let payload_bytes = reader.remaining_slice();
        Self::new(selector, payload_bytes)
    }

    pub fn selector(&self) -> u64 {
        self.selector
    }

    pub fn payload(&self) -> &[u8] {
        self.payload.as_slice()
    }

    pub fn try_as<T: Exn>(&self) -> Option<T> {
        if self.selector == T::SELECTOR {
            // no auth handles on exception handling path
            contract_call_decode_exn_prefix(self.payload.as_slice()).ok()
        } else {
            None
        }
    }

    /// Re-throw this exception to our caller, reconstructing the wire format.
    #[inline]
    pub fn re_throw(&self) -> ! {
        crate::contract_abort_encoded(self)
    }
}

/// Serializes as [CXS(selector)][raw payload bytes] — the same wire format
/// produced by `ContractExn<E>`. The payload is already CXS-encoded, so it's
/// written directly without a length prefix.
impl WriteXDR for RawException {
    fn write_xdr<F, W: Writer<F>>(&self, w: &mut W) -> WriteResult<()> {
        self.selector.write_xdr(w)?;
        w.write_opaque_fixed(self.payload.as_slice())
    }
}

impl Debug for RawException {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "RawException {{ selector: 0x{:016X}, payload: {} bytes }}",
            self.selector,
            self.payload.len()
        )
    }
}

impl PartialEq for RawException {
    fn eq(&self, other: &Self) -> bool {
        self.selector == other.selector && self.payload == other.payload
    }
}

impl crate::re_throw::ReThrow for RawException {
    fn re_throw(self) -> ! {
        // Delegate to the existing inherent re_throw(&self) method
        RawException::re_throw(&self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::internal::encode_xdr;

    #[derive(Debug, Clone, PartialEq, ReadXDR, WriteXDR, thiserror::Error)]
    #[repr(i32)]
    enum FakeError {
        #[error("fake")]
        Fake = 1,
    }

    impl Exn for FakeError {
        const SELECTOR: u64 = 0x1234_5678_9ABC_DEF0;
    }

    #[test]
    fn try_as_matching_selector_decodes() {
        let encoded = encode_xdr(&FakeError::Fake).unwrap();
        let raw =
            RawException::new(FakeError::SELECTOR, encoded.as_ref()).unwrap();
        assert_eq!(raw.try_as::<FakeError>(), Some(FakeError::Fake));
    }

    #[test]
    fn try_as_wrong_selector_returns_none() {
        let encoded = encode_xdr(&FakeError::Fake).unwrap();
        let raw = RawException::new(0xFFFF, encoded.as_ref()).unwrap();
        assert_eq!(raw.try_as::<FakeError>(), None);
    }

    #[test]
    fn try_as_corrupt_payload_returns_none() {
        let raw = RawException::new(FakeError::SELECTOR, &[0xFF, 0xFF, 0xFF])
            .unwrap();
        assert_eq!(raw.try_as::<FakeError>(), None);
    }

    #[test]
    fn from_abort_buf_round_trips() {
        let encoded = encode_xdr(&FakeError::Fake).unwrap();
        // Build a wire-format buffer: [selector][payload]
        let mut wire = heapless::Vec::<u8, 500>::new();
        let written =
            cxs::encode_into(&FakeError::SELECTOR, wire.spare_capacity_mut())
                .unwrap();
        unsafe { wire.set_len(written) };
        wire.extend_from_slice(encoded.as_ref()).ok();

        let raw = RawException::from_abort_buf(&wire).unwrap();
        assert_eq!(raw.selector(), FakeError::SELECTOR);
        assert_eq!(raw.try_as::<FakeError>(), Some(FakeError::Fake));
    }

    /// Under `alloc` the payload buffer grows to fit, so a payload far larger
    /// than the no-alloc `SCRATCH_MAX` cap round-trips intact (no truncation).
    ///
    /// The complementary no-alloc behavior — a payload exceeding `SCRATCH_MAX`
    /// returning `Err` instead of a truncated value — is only reachable on the
    /// guest target; the host test harness is always built with `alloc`, so it
    /// cannot exercise that branch here.
    #[cfg(feature = "alloc")]
    #[test]
    fn new_holds_oversized_payload_without_truncation_under_alloc() {
        let big = [0xABu8; crate::internal::SCRATCH_MAX + 1];
        let raw = RawException::new(0x1234, &big).unwrap();
        assert_eq!(raw.payload(), &big[..]);
    }
}
