use crate::call::internal::contract_call_decode;
use crate::exn::Exn;
use core::fmt::{self, Debug};

/// An unrecognized error from a contract call.
///
/// Contains the raw selector and XDR payload bytes for errors that
/// don't match any declared throws type. Use `try_as::<T>()` to
/// attempt decoding as a known type.
pub struct RawError {
    selector: u64,
    payload: heapless::Vec<u8, 500>,
}

impl RawError {
    pub fn new(selector: u64, payload: &[u8]) -> Self {
        let mut buf = heapless::Vec::new();
        buf.extend_from_slice(payload).ok();
        Self { selector, payload: buf }
    }

    pub fn selector(&self) -> u64 { self.selector }

    pub fn payload(&self) -> &[u8] { &self.payload }

    pub fn try_as<T: Exn>(&self) -> Option<T> {
        if self.selector == T::SELECTOR {
            contract_call_decode(&self.payload).ok()
        } else {
            None
        }
    }
}

impl Debug for RawError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "RawError {{ selector: 0x{:016X}, payload: {} bytes }}", self.selector, self.payload.len())
    }
}

impl PartialEq for RawError {
    fn eq(&self, other: &Self) -> bool {
        self.selector == other.selector && self.payload == other.payload
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::call::internal::contract_call_encode;
    use crate::xdr::*;

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
        let raw = RawError::new(FakeError::SELECTOR, &encoded);
        assert_eq!(raw.try_as::<FakeError>(), Some(FakeError::Fake));
    }

    #[test]
    fn try_as_wrong_selector_returns_none() {
        let encoded = contract_call_encode(&FakeError::Fake).unwrap();
        let raw = RawError::new(0xFFFF, &encoded);
        assert_eq!(raw.try_as::<FakeError>(), None);
    }

    #[test]
    fn try_as_corrupt_payload_returns_none() {
        let raw = RawError::new(FakeError::SELECTOR, &[0xFF, 0xFF, 0xFF]);
        assert_eq!(raw.try_as::<FakeError>(), None);
    }
}
