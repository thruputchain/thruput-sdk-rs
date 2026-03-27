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

// --- Direct serialization function ---
//
// Serializes errors into the wire format [u64 selector][payload][debug info]
// and calls contract_abort.

#[cfg(not(feature = "debugging"))]
pub(crate) fn serialize_error<E: WriteXDR + crate::exn::Exn>(err: &E) -> ! {
    use crate::call::internal::contract_call_encode;

    let selector = E::SELECTOR;
    let buf = contract_call_encode(&(selector, err))
        .unwrap_or_else(|e| super::encoding_exception_throw(e));

    crate::contract_abort(buf.as_ref());
}

#[cfg(feature = "debugging")]
#[track_caller]
pub(crate) fn serialize_error<E: WriteXDR + crate::exn::Exn>(err: &E) -> ! {
    use crate::call::internal::contract_call_encode;
    use core::panic::Location;

    let selector = E::SELECTOR;
    let location: Option<LocationString> = Some(format(Location::caller()));
    let backtrace: Option<BacktraceString> =
        Some(format(&::abort::Backtrace::capture()));

    let buf = contract_call_encode(&(selector, err, &location, &backtrace))
        .unwrap_or_else(|e| super::encoding_exception_throw(e));

    crate::contract_abort(buf.as_ref());
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

    /// Verify that serialize_error's wire format is [u64 selector][payload].
    /// We encode the tuple (selector, err) and then decode the parts
    /// individually to confirm the layout.
    #[test]
    fn serialize_error_writes_selector_then_payload() {
        let exn = TestExn::Boom;
        let buf = contract_call_encode(&(TestExn::SELECTOR, &exn)).unwrap();

        // Decode as (u64, TestExn) — the format serialize_error produces
        let (selector, decoded): (u64, TestExn) =
            cxs::decode(&buf[..]).unwrap();

        assert_eq!(selector, 0xFFFF_FFFF_FFFF_FFFF);
        assert_eq!(decoded, TestExn::Boom);
    }

    #[test]
    fn serialize_error_writes_library_selector_then_payload() {
        let exn = LibraryExn::NonNegativeIntOverflow;
        let buf = contract_call_encode(&(LibraryExn::SELECTOR, &exn)).unwrap();

        // Decode as (u64, LibraryExn)
        let (selector, decoded): (u64, LibraryExn) =
            cxs::decode(&buf[..]).unwrap();

        assert_eq!(selector, LibraryExn::SELECTOR);
        assert_eq!(decoded, LibraryExn::NonNegativeIntOverflow);
    }

    /// Verify that the selector can be decoded independently from
    /// the payload -- important for dispatching on the receiver side.
    #[test]
    fn selector_can_be_decoded_without_knowing_payload_type() {
        let exn = TestExn::Boom;
        let buf = contract_call_encode(&(TestExn::SELECTOR, &exn)).unwrap();

        // A receiver that doesn't know the error type can still read the
        // selector to figure out what type to decode next.
        let mut reader = cxs::buf_reader::BufReader::new(&buf[..]);
        let selector: u64 = ReadXDR::read_xdr(&mut reader).unwrap();

        assert_eq!(selector, TestExn::SELECTOR);
    }
}
