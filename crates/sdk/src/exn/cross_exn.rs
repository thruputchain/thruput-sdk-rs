//! Exceptions passed across contract calls.

use crate::xdr::*;

/// A typed exception ready for serialization across the contract boundary.
///
/// Wire format: [u64 selector][E payload]
#[derive(WriteXDR, Debug)]
pub(crate) struct ContractExn<E: WriteXDR> {
    selector: u64,
    exn: E,
}

impl<E: WriteXDR + crate::exn::Exn> ContractExn<E> {
    #[cfg_attr(not(target_arch = "riscv32"), track_caller)]
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
    use crate::exn::{Exn, LibraryExn};
    use crate::internal::encode_xdr;

    // A contract-level exception type for testing
    #[derive(Debug, Clone, PartialEq, ReadXDR, WriteXDR, thiserror::Error)]
    #[repr(i32)]
    enum TestExn {
        #[error("boom")]
        Boom = 1,
        #[error("data")]
        Data(u32) = 2,
    }

    impl Exn for TestExn {
        const SELECTOR: u64 = 0xFFFF_FFFF_FFFF_FFFF; // placeholder for tests
    }

    /// Verify that ContractExn's wire format is [u64 selector][payload].
    /// We encode the ContractExn and then decode the selector and payload
    /// individually to confirm the layout.
    #[test]
    fn contract_exn_writes_selector_then_payload() {
        let exn = ContractExn::new(TestExn::Boom);
        let buf = encode_xdr(&exn).unwrap();

        // Decode selector then payload using a reader
        let mut reader = cxs::buf_reader::BufReader::new(buf.as_ref());
        let selector: u64 = ReadXDR::read_xdr(&mut reader).unwrap();
        let decoded: TestExn = ReadXDR::read_xdr(&mut reader).unwrap();

        assert_eq!(selector, 0xFFFF_FFFF_FFFF_FFFF);
        assert_eq!(decoded, TestExn::Boom);
    }

    #[test]
    fn contract_exn_writes_library_selector_then_payload() {
        let exn = ContractExn::new(LibraryExn::NonNegativeIntOverflow);
        let buf = encode_xdr(&exn).unwrap();

        // Decode selector then payload using a reader
        let mut reader = cxs::buf_reader::BufReader::new(buf.as_ref());
        let selector: u64 = ReadXDR::read_xdr(&mut reader).unwrap();
        let decoded: LibraryExn = ReadXDR::read_xdr(&mut reader).unwrap();

        assert_eq!(selector, LibraryExn::SELECTOR);
        assert_eq!(decoded, LibraryExn::NonNegativeIntOverflow);
    }

    /// A reference throws as its referent: same selector, byte-identical
    /// wire encoding (including payload bytes), no clone required.
    #[test]
    fn contract_exn_by_reference_encodes_identically_to_owned() {
        let exn = TestExn::Data(0xDEAD_BEEF);
        let by_ref = encode_xdr(&ContractExn::new(&exn)).unwrap();
        let owned = encode_xdr(&ContractExn::new(exn.clone())).unwrap();
        assert_eq!(by_ref, owned);

        // And a receiver can decode the by-ref encoding back to the
        // owned payload.
        let mut reader = cxs::buf_reader::BufReader::new(by_ref.as_ref());
        let selector: u64 = ReadXDR::read_xdr(&mut reader).unwrap();
        let decoded: TestExn = ReadXDR::read_xdr(&mut reader).unwrap();
        assert_eq!(selector, TestExn::SELECTOR);
        assert_eq!(decoded, exn);
    }

    /// Verify that the selector can be decoded independently from
    /// the payload -- important for dispatching on the receiver side.
    #[test]
    fn selector_can_be_decoded_without_knowing_payload_type() {
        let exn = ContractExn::new(TestExn::Boom);
        let buf = encode_xdr(&exn).unwrap();

        // A receiver that doesn't know the error type can still read the
        // selector to figure out what type to decode next.
        let mut reader = cxs::buf_reader::BufReader::new(buf.as_ref());
        let selector: u64 = ReadXDR::read_xdr(&mut reader).unwrap();

        assert_eq!(selector, TestExn::SELECTOR);
    }
}
