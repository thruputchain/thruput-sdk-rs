//! Exception trait and the borrowed exception type, split out of the SDK
//! (like `thruput-event`) so host-side tooling below the SDK can decode
//! thrown exceptions.

#![no_std]

use core::fmt::Debug;
use xdrs::{ReadError, ReadXDR, ReadXDROwned, WriteXDR, cxs};

/// A trait for exception types that can be serialized across contract calls.
///
/// Each implementing type gets a unique u64 selector derived from its
/// canonical type signature (via `#[exn]`). The selector is used
/// for dispatch when deserializing exceptions on the client side.
///
/// Throwing only needs `WriteXDR`, so decoding is not a supertrait; the
/// generic decode sites ([`RawExceptionRef::try_as`] and the SDK's
/// `RawException::try_as`) require `ReadXDROwned` explicitly.
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

/// A raw exception borrowed from an abort buffer, `[CXS(selector)][payload]`.
///
/// In the SDK this only exists inside the `on_exn` dispatch closure of
/// `try_call_raw_with`, borrowing the call scratch for exactly the closure's
/// lifetime; `RawException` is its owned form. On the host, the mock
/// executor parses one straight from a frame's abort buffer.
///
/// `Copy`: all three fields are, and `MatchExn` hands out reborrows of it.
/// Covariance in `'s` makes a copy usable at any shorter lifetime.
#[derive(Clone, Copy)]
pub struct RawExceptionRef<'s> {
    selector: u64,
    /// Full verbatim abort wire bytes: `[CXS(selector)][payload]`.
    wire: &'s [u8],
    payload_start: usize,
}

impl<'s> RawExceptionRef<'s> {
    /// Fails only when the buffer does not start with a CXS-encoded `u64`:
    /// the frame aborted with something other than a contract exception.
    pub fn parse(wire: &'s [u8]) -> Result<Self, ReadError> {
        let mut reader = cxs::buf_reader::BufReader::new(wire);
        let selector: u64 = ReadXDR::read_xdr(&mut reader)?;
        Ok(Self {
            selector,
            wire,
            payload_start: wire.len() - reader.remaining_slice().len(),
        })
    }

    /// Reassemble from parts an earlier [`parse`](Self::parse) produced, for
    /// an owner of a copied wire buffer that kept `selector` and
    /// `payload_start` alongside it. The SDK's owned `RawException` is the
    /// intended caller; the offset is only checked in debug builds.
    #[doc(hidden)]
    pub fn from_parts(
        selector: u64,
        wire: &'s [u8],
        payload_start: usize,
    ) -> Self {
        debug_assert!(
            payload_start <= wire.len(),
            "payload_start {payload_start} past wire end {}",
            wire.len()
        );
        Self {
            selector,
            wire,
            payload_start,
        }
    }

    pub fn selector(&self) -> u64 {
        self.selector
    }

    /// The full abort wire bytes, selector prefix included.
    pub fn wire(&self) -> &'s [u8] {
        self.wire
    }

    pub fn payload(&self) -> &'s [u8] {
        &self.wire[self.payload_start..]
    }

    /// Where [`payload`](Self::payload) starts within [`wire`](Self::wire).
    pub fn payload_start(&self) -> usize {
        self.payload_start
    }

    /// Detach into an owned exception (one copy). Which owned type is up to
    /// the caller's context; the SDK's `RawException` is the one that
    /// exists, and this resolves through its `From` impl.
    pub fn to_owned<O: From<Self>>(self) -> O {
        O::from(self)
    }

    /// Decode as `T` if the selector matches, `None` otherwise.
    ///
    /// `None` covers two different situations, a selector belonging to some
    /// other type, and this type's selector over a payload that will not
    /// decode. Use [`try_decode`](Self::try_decode) when they need telling
    /// apart.
    ///
    /// `T` must contain no auth-token handle (the SDK's `Signed`): abort
    /// frames carry none, so one would decode to the uninitialized
    /// `u32::MAX` sentinel, a value shaped like an auth token that faults on
    /// use. Nothing is checked here; spec canonicalization rejects such a
    /// payload when the contract is built.
    pub fn try_as<T: Exn + ReadXDROwned>(&self) -> Option<T> {
        // Defined in terms of `try_decode` so the two cannot drift on which
        // payloads they accept.
        self.try_decode().and_then(|decoded| decoded.ok())
    }

    /// [`try_as`](Self::try_as) with the decode failure kept separate from
    /// the selector mismatch:
    ///
    /// - `None`: the selector belongs to some other exception type.
    /// - `Some(Ok(t))`: the selector matched and the payload decoded.
    /// - `Some(Err(e))`: the selector matched, but the payload does not
    ///   decode as `T`.
    ///
    /// The third case is not only a forged or truncated payload. An
    /// exception's selector is `sha256(major_version :: crate :: type)` and
    /// says nothing about the payload's layout, so a callee that adds an
    /// enum variant (a semver-compatible change that leaves the selector
    /// untouched) throws something an older caller cannot decode. Since
    /// [`try_as`] reports that as `None`, indistinguishable from an
    /// unrelated error type, a caller that must not confuse the two should
    /// reach for this instead.
    ///
    /// Note that decoding reads a *prefix*: a payload carrying extra
    /// trailing bytes decodes as the shorter shape and is reported as
    /// `Some(Ok(..))`. Appending a field is therefore accepted silently
    /// rather than surfacing here.
    ///
    /// [`try_as`]: Self::try_as
    pub fn try_decode<T: Exn + ReadXDROwned>(
        &self,
    ) -> Option<Result<T, ReadError>> {
        if self.selector != T::SELECTOR {
            return None;
        }
        let mut reader = cxs::buf_reader::BufReader::new(self.payload());
        Some(ReadXDR::read_xdr(&mut reader))
    }
}

impl core::fmt::Debug for RawExceptionRef<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let payload = self.payload();
        write!(
            f,
            "RawExceptionRef {{ selector: 0x{:016X}, payload: {} bytes",
            self.selector,
            payload.len(),
        )?;
        #[cfg(not(target_arch = "riscv32"))]
        {
            write!(f, ", hex: ")?;
            for b in payload {
                write!(f, "{b:02x}")?;
            }
        }
        write!(f, " }}")
    }
}

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec::Vec;

    #[derive(Debug, PartialEq, xdrs::WriteXDR, xdrs::ReadXDR)]
    struct TestExn(u32);

    impl Exn for TestExn {
        const SELECTOR: u64 = 0xABCD;
    }

    fn wire(selector: u64, payload: &[u8]) -> Vec<u8> {
        let mut wire = cxs::encode(&selector).unwrap();
        wire.extend_from_slice(payload);
        wire
    }

    #[test]
    fn parse_separates_selector_and_payload() {
        let wire = wire(0xABCD, b"payload-bytes");
        let exn = RawExceptionRef::parse(&wire).unwrap();
        assert_eq!(exn.selector(), 0xABCD);
        assert_eq!(exn.payload(), b"payload-bytes");
        assert_eq!(exn.wire(), &wire[..]);
        assert_eq!(exn.payload_start(), wire.len() - b"payload-bytes".len());
    }

    #[test]
    fn parse_rejects_garbage() {
        assert!(RawExceptionRef::parse(&[]).is_err());
    }

    #[test]
    fn try_decode_distinguishes_selector_from_payload_failures() {
        let payload = cxs::encode(&7u32).unwrap();
        let ok = wire(TestExn::SELECTOR, &payload);
        assert!(matches!(
            RawExceptionRef::parse(&ok).unwrap().try_decode::<TestExn>(),
            Some(Ok(TestExn(7)))
        ));
        assert_eq!(
            RawExceptionRef::parse(&ok).unwrap().try_as::<TestExn>(),
            Some(TestExn(7))
        );

        let other = wire(TestExn::SELECTOR + 1, &payload);
        assert!(
            RawExceptionRef::parse(&other)
                .unwrap()
                .try_decode::<TestExn>()
                .is_none()
        );

        let truncated = wire(TestExn::SELECTOR, &[]);
        assert!(matches!(
            RawExceptionRef::parse(&truncated)
                .unwrap()
                .try_decode::<TestExn>(),
            Some(Err(_))
        ));
        assert_eq!(
            RawExceptionRef::parse(&truncated)
                .unwrap()
                .try_as::<TestExn>(),
            None
        );
    }
}
