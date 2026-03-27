//! Exceptions passed across contract calls.

use crate::xdr::*;
#[cfg(feature = "debugging")]
use core::fmt::Write;

#[cfg(feature = "debugging")]
type LocationString = heapless::String<200>;

#[cfg(feature = "debugging")]
type BacktraceString = heapless::String<500>;

#[cfg(feature = "debugging")]
fn format<T, const N: usize>(v: &T) -> heapless::String<N>
where
    T: core::fmt::Display,
{
    let mut s = heapless::String::<N>::new();

    write!(&mut s, "{}", v).unwrap(); // XXX

    s
}

/// A typed exception ready for serialization across the contract boundary.
///
/// Wire format: [u64 selector][E payload][debug location?][debug backtrace?]
#[derive(WriteXDR)]
pub(crate) struct ContractExn<E: WriteXDR> {
    selector: u64,
    exn: E,
    #[cfg(feature = "debugging")]
    location: Option<LocationString>,
    #[cfg(feature = "debugging")]
    backtrace: Option<BacktraceString>,
}

impl<E: WriteXDR + crate::exn::Exn> ContractExn<E> {
    #[cfg(feature = "debugging")]
    #[track_caller]
    pub(crate) fn new(exn: E) -> Self {
        use core::panic::Location;
        Self {
            selector: E::SELECTOR,
            exn,
            location: Some(format(Location::caller())),
            backtrace: Some(format(&::abort::Backtrace::capture())),
        }
    }

    #[cfg(not(feature = "debugging"))]
    pub(crate) fn new(exn: E) -> Self {
        Self {
            selector: E::SELECTOR,
            exn,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::call::internal::contract_call_encode;
    use crate::exn::{Exn, LibraryExn};

    // A contract-level exception type for testing
    #[derive(Debug, Clone, PartialEq, ReadXDR, WriteXDR, thiserror::Error)]
    #[repr(i32)]
    enum TestExn {
        #[error("boom")]
        Boom = 1,
    }

    impl Exn for TestExn {
        const SELECTOR: u64 = 0xFFFF_FFFF_FFFF_FFFF; // placeholder for tests
    }

    /// Verify that ContractExn's wire format starts with [u64 selector][payload].
    /// We encode the ContractExn and then decode the selector and payload
    /// individually to confirm the layout. Uses prefix decoding because the
    /// `debugging` feature appends location/backtrace after the payload.
    #[test]
    fn contract_exn_writes_selector_then_payload() {
        let exn = ContractExn::new(TestExn::Boom);
        let buf = contract_call_encode(&exn).unwrap();

        // Decode selector then payload using a reader (tolerates trailing debug info)
        let mut reader = cxs::buf_reader::BufReader::new(&buf[..]);
        let selector: u64 = ReadXDR::read_xdr(&mut reader).unwrap();
        let decoded: TestExn = ReadXDR::read_xdr(&mut reader).unwrap();

        assert_eq!(selector, 0xFFFF_FFFF_FFFF_FFFF);
        assert_eq!(decoded, TestExn::Boom);
    }

    #[test]
    fn contract_exn_writes_library_selector_then_payload() {
        let exn = ContractExn::new(LibraryExn::NonNegativeIntOverflow);
        let buf = contract_call_encode(&exn).unwrap();

        // Decode selector then payload using a reader (tolerates trailing debug info)
        let mut reader = cxs::buf_reader::BufReader::new(&buf[..]);
        let selector: u64 = ReadXDR::read_xdr(&mut reader).unwrap();
        let decoded: LibraryExn = ReadXDR::read_xdr(&mut reader).unwrap();

        assert_eq!(selector, LibraryExn::SELECTOR);
        assert_eq!(decoded, LibraryExn::NonNegativeIntOverflow);
    }

    /// Verify that the selector can be decoded independently from
    /// the payload -- important for dispatching on the receiver side.
    #[test]
    fn selector_can_be_decoded_without_knowing_payload_type() {
        let exn = ContractExn::new(TestExn::Boom);
        let buf = contract_call_encode(&exn).unwrap();

        // A receiver that doesn't know the error type can still read the
        // selector to figure out what type to decode next.
        let mut reader = cxs::buf_reader::BufReader::new(&buf[..]);
        let selector: u64 = ReadXDR::read_xdr(&mut reader).unwrap();

        assert_eq!(selector, TestExn::SELECTOR);
    }
}
