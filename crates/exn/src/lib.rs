//! Exception trait and abort wire format, split out of the SDK (like
//! `groundhog-event`) so host-side tooling below the SDK can read selectors
//! and abort buffers.

#![no_std]

use core::fmt::Debug;
use xdrs::{ReadError, ReadXDR, WriteXDR, cxs};

/// A trait for exception types that can be serialized across contract calls.
///
/// Each implementing type gets a unique u64 selector derived from its
/// canonical type signature (via `#[exn]`). The selector is used
/// for dispatch when deserializing exceptions on the client side.
///
/// Throwing only needs `WriteXDR`, so decoding is not a supertrait; the
/// generic decode sites (`RawExceptionRef::try_as` / `RawException::try_as`
/// in the SDK) require `ReadXDROwned` explicitly.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a contract exception",
    label = "not a contract exception",
    note = "annotate `{Self}` with `#[contract_exception]` (or `#[exn]` if \
            you handle your own derives); `throws = ...` in \
            `#[contract(...)]` requires it"
)]
pub trait Exn: Debug + PartialEq + WriteXDR {
    const SELECTOR: u64;
}

/// A reference throws as its referent: same selector, same wire encoding
/// (`WriteXDR for &T` writes `T`). Lets throw paths borrow an exception
/// instead of cloning it.
impl<T: Exn> Exn for &T {
    const SELECTOR: u64 = T::SELECTOR;
}

/// Exn impl for Infallible: this type can never be instantiated,
/// so its selector is 0 (a reserved/unused value).
impl Exn for core::convert::Infallible {
    const SELECTOR: u64 = 0;
}

/// Split a thrown exception's abort wire buffer, `[CXS(selector)][payload]`,
/// into its selector and payload bytes.
///
/// Fails only when the buffer does not start with a CXS-encoded `u64`: the
/// frame aborted with something other than a contract exception.
pub fn split_abort_wire(wire: &[u8]) -> Result<(u64, &[u8]), ReadError> {
    let mut reader = cxs::buf_reader::BufReader::new(wire);
    let selector: u64 = ReadXDR::read_xdr(&mut reader)?;
    Ok((selector, reader.remaining_slice()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_abort_wire_separates_selector_and_payload() {
        let mut wire = cxs::encode(&0xABCDu64).unwrap();
        wire.extend_from_slice(b"payload-bytes");
        let (selector, payload) = split_abort_wire(&wire).unwrap();
        assert_eq!(selector, 0xABCD);
        assert_eq!(payload, b"payload-bytes");
    }

    #[test]
    fn split_abort_wire_rejects_garbage() {
        assert!(split_abort_wire(&[]).is_err());
    }
}
