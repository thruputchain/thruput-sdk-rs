use crate::call::internal::contract_call_decode_prefix;
use crate::exn::Exn;
use crate::xdr::*;
use core::fmt::{self, Debug};

/// A raw exception received from a contract call.
///
/// Contains the selector and remaining payload bytes. Use `try_as::<T>()`
/// to attempt decoding as a known exception type.
pub struct RawException {
    selector: u64,
    payload: heapless::Vec<u8, 500>,
}

impl RawException {
    pub fn new(selector: u64, payload: &[u8]) -> Self {
        let mut buf = heapless::Vec::new();
        buf.extend_from_slice(payload).ok();
        Self {
            selector,
            payload: buf,
        }
    }

    /// Decode a RawException from an abort buffer.
    /// The buffer is CXS-encoded: [u64 selector][payload bytes...].
    pub fn from_abort_buf(
        bytes: &[u8],
    ) -> Result<Self, ReadError> {
        let mut reader = cxs::buf_reader::BufReader::new(bytes);
        let selector: u64 = ReadXDR::read_xdr(&mut reader)?;
        let payload_bytes = &bytes[reader.position()..];
        Ok(Self::new(selector, payload_bytes))
    }

    pub fn selector(&self) -> u64 {
        self.selector
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    pub fn try_as<T: Exn>(&self) -> Option<T> {
        if self.selector == T::SELECTOR {
            contract_call_decode_prefix(&self.payload).ok()
        } else {
            None
        }
    }

    /// Re-throw this exception to our caller, reconstructing the wire format.
    pub fn re_throw(&self) -> ! {
        let mut buf = crate::call::AbortBuf::new();
        let written =
            cxs::encode_into(&self.selector, buf.spare_capacity_mut())
                .unwrap_or_else(|_| {
                    panic!("re_throw: couldn't encode selector")
                });
        unsafe { buf.set_len(written) };
        buf.extend_from_slice(&self.payload).ok();
        crate::contract_abort(buf.as_ref());
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::call::internal::contract_call_encode;

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
        let encoded = contract_call_encode(&FakeError::Fake).unwrap();
        let raw = RawException::new(FakeError::SELECTOR, &encoded);
        assert_eq!(raw.try_as::<FakeError>(), Some(FakeError::Fake));
    }

    #[test]
    fn try_as_wrong_selector_returns_none() {
        let encoded = contract_call_encode(&FakeError::Fake).unwrap();
        let raw = RawException::new(0xFFFF, &encoded);
        assert_eq!(raw.try_as::<FakeError>(), None);
    }

    #[test]
    fn try_as_corrupt_payload_returns_none() {
        let raw =
            RawException::new(FakeError::SELECTOR, &[0xFF, 0xFF, 0xFF]);
        assert_eq!(raw.try_as::<FakeError>(), None);
    }

    #[test]
    fn from_abort_buf_round_trips() {
        let encoded = contract_call_encode(&FakeError::Fake).unwrap();
        // Build a wire-format buffer: [selector][payload]
        let mut wire = heapless::Vec::<u8, 500>::new();
        let written = cxs::encode_into(
            &FakeError::SELECTOR,
            wire.spare_capacity_mut(),
        )
        .unwrap();
        unsafe { wire.set_len(written) };
        wire.extend_from_slice(&encoded).ok();

        let raw = RawException::from_abort_buf(&wire).unwrap();
        assert_eq!(raw.selector(), FakeError::SELECTOR);
        assert_eq!(raw.try_as::<FakeError>(), Some(FakeError::Fake));
    }
}
