//! The compile-time [`DomainSeparator`] type.

use xdrs::{ConstWriteXDR, WriteResult, WriteXDR, Writer, WriterUnchecked};

/// A compile-time domain separator: the CXS bytes of
/// `DomainSeparator { ext: 0, domain_separator: <name> }`, held in rodata.
///
/// The crate's separators are declared as constants (see [`PUBKEY_ACCOUNT`]);
/// the prefix bytes are computed at compile time.
pub struct DomainSeparator {
    cxs: &'static [u8],
}

impl DomainSeparator {
    /// Wraps precomputed CXS prefix bytes.
    pub(crate) const fn from_cxs_bytes(cxs: &'static [u8]) -> Self {
        Self { cxs }
    }

    /// The CXS-encoded domain-separator prefix bytes.
    pub const fn as_bytes(&self) -> &'static [u8] {
        self.cxs
    }
}

/// Writes the already-encoded prefix bytes verbatim, so a `DomainSeparator`
/// can be a field of a `#[derive(WriteXDR)]` struct (see `HashPreimage`) and
/// the preimage layout is spelled exactly once.
///
/// CXS-ONLY. `self.cxs` holds bytes that were already CXS-encoded at compile
/// time, and under CXS `write_opaque_fixed` is a verbatim `put_bytes` — no
/// length prefix, no padding — so re-emitting them through it is a faithful
/// pass-through. `write_xdr` is generic over the format, but this impl is
/// **only correct for CXS**: classic XDR pads `write_opaque_fixed` to a 4-byte
/// boundary, which would silently corrupt the 15-byte prefix. Only ever reach
/// this impl via `xdrs::cxs::encode`/`encode_into`.
impl WriteXDR for DomainSeparator {
    #[inline]
    fn write_xdr<F, W: Writer<F>>(&self, w: &mut W) -> WriteResult<()> {
        w.write_opaque_fixed(self.cxs)
    }

    #[inline]
    unsafe fn write_xdr_unchecked<F, W: WriterUnchecked<F>>(&self, w: &mut W) {
        // SAFETY: forwarded caller contract.
        unsafe { w.write_opaque_fixed(self.cxs) }
    }
}

/// Mirror of `groundhog_xdr::preimage::DomainSeparator` whose opaque field is a
/// borrow (`&[u8]`) rather than `BytesM<256>`, so a value can be built in a
/// `const` (the real type's `BytesM<256>` is heap-backed under `alloc`). It
/// encodes byte-identically; [`DomainSeparatorRepr::to_domain_separator`] and a
/// test pin it to the real type.
///
/// Crate-internal: this exists only to compute prefix constants.
#[derive(ConstWriteXDR, WriteXDR)]
pub(crate) struct DomainSeparatorRepr<'a> {
    /// `ExtensionPoint::_0` discriminant (always 0).
    pub ext: i32,
    /// `domain_separator: opaque<256>`.
    pub name: &'a [u8],
}

impl DomainSeparatorRepr<'_> {
    /// Build the real protocol `DomainSeparator`. The exhaustive struct literal
    /// makes any change to `DomainSeparator`'s fields or field types fail to
    /// compile here, keeping this mirror in lockstep with the real type.
    ///
    /// This is a **compile-time drift guard**: its value is being type-checked
    /// on every `alloc` build, not being called. Only the test below calls it,
    /// hence the `allow(dead_code)` — removing it would weaken the guard to
    /// test-only compilation.
    #[cfg(feature = "alloc")]
    #[allow(dead_code)]
    pub(crate) fn to_domain_separator(
        &self,
    ) -> groundhog_xdr::preimage::DomainSeparator {
        groundhog_xdr::preimage::DomainSeparator {
            ext: groundhog_xdr::types::ExtensionPoint::_0,
            domain_separator: xdrs::BytesM::<256>::try_from(self.name)
                .expect("domain separator name exceeds BytesM<256>"),
        }
    }
}

/// Computes the CXS-encoded size of a [`DomainSeparatorRepr`].
pub(crate) const fn prefix_size(repr: &DomainSeparatorRepr) -> usize {
    repr.const_cxs_size()
}

/// CXS-encodes a [`DomainSeparatorRepr`] into a fixed `[u8; N]`.
pub(crate) const fn prefix_bytes<const N: usize>(
    repr: &DomainSeparatorRepr,
) -> [u8; N] {
    let mut w = xdrs::cxs::ConstWriter::<N>::new();
    repr.const_write_cxs(&mut w);
    w.into_bytes()
}

// --- Separators -------------------------------------------------------------
//
// Each separator is three consts: the mirror value, its CXS size, and the
// encoded bytes. Declaring one by hand is a few lines; if enough of them
// accumulate to want a macro, note that a `macro_rules!` would force
// `DomainSeparatorRepr` and the two helpers above to be `pub` again (macros
// expand at the call site), which is why there isn't one for a single
// separator.

const PUBKEY_ACCOUNT_NAME: &[u8] = b"PubkeyAccount";

const _: () = ::core::assert!(
    PUBKEY_ACCOUNT_NAME.len() <= 256,
    "domain separator name exceeds BytesM<256>"
);

const PUBKEY_ACCOUNT_REPR: DomainSeparatorRepr<'static> =
    DomainSeparatorRepr {
        ext: 0,
        name: PUBKEY_ACCOUNT_NAME,
    };

const PUBKEY_ACCOUNT_N: usize = prefix_size(&PUBKEY_ACCOUNT_REPR);

const PUBKEY_ACCOUNT_BYTES: [u8; PUBKEY_ACCOUNT_N] =
    prefix_bytes::<PUBKEY_ACCOUNT_N>(&PUBKEY_ACCOUNT_REPR);

/// Domain separator for pubkey account address derivation (`"PubkeyAccount"`).
pub const PUBKEY_ACCOUNT: DomainSeparator =
    DomainSeparator::from_cxs_bytes(&PUBKEY_ACCOUNT_BYTES);

const TRANSACTION_NAME: &[u8] = b"Transaction";

const _: () = ::core::assert!(
    TRANSACTION_NAME.len() <= 256,
    "domain separator name exceeds BytesM<256>"
);

const TRANSACTION_REPR: DomainSeparatorRepr<'static> = DomainSeparatorRepr {
    ext: 0,
    name: TRANSACTION_NAME,
};

const TRANSACTION_N: usize = prefix_size(&TRANSACTION_REPR);

const TRANSACTION_BYTES: [u8; TRANSACTION_N] =
    prefix_bytes::<TRANSACTION_N>(&TRANSACTION_REPR);

/// Domain separator for transaction signature preimages (`"Transaction"`).
///
/// Unlike [`PUBKEY_ACCOUNT`] this one prefixes a message that is signed
/// directly rather than hashed; see `transaction_signature_preimage`.
pub const TRANSACTION: DomainSeparator =
    DomainSeparator::from_cxs_bytes(&TRANSACTION_BYTES);

#[cfg(all(test, feature = "alloc", feature = "sha2"))]
mod tests {
    use super::*;

    #[test]
    fn prefix_matches_real_domain_separator() {
        // Build the mirror for the same name and encode the REAL type from it;
        // the const prefix must equal the real CXS encoding.
        let repr = DomainSeparatorRepr {
            ext: 0,
            name: b"PubkeyAccount",
        };
        let real = repr.to_domain_separator();
        let encoded =
            xdrs::cxs::encode(&real).expect("encode DomainSeparator");
        assert_eq!(PUBKEY_ACCOUNT.as_bytes(), encoded.as_slice());
    }

    #[test]
    fn prefix_expected_bytes() {
        // 0x00 (ext discriminant) + 0x0d (opaque len = 13) + "PubkeyAccount".
        assert_eq!(PUBKEY_ACCOUNT.as_bytes(), &b"\x00\x0dPubkeyAccount"[..]);
        // 0x00 (ext discriminant) + 0x0b (opaque len = 11) + "Transaction".
        assert_eq!(TRANSACTION.as_bytes(), &b"\x00\x0bTransaction"[..]);
    }

    #[test]
    fn separators_are_distinct() {
        assert_ne!(PUBKEY_ACCOUNT.as_bytes(), TRANSACTION.as_bytes());
    }

    /// The `WriteXDR` pass-through must re-emit the prefix verbatim — no length
    /// prefix, no padding — or the derived `HashPreimage` layout would shift.
    #[test]
    fn write_xdr_reemits_prefix_verbatim() {
        let encoded =
            xdrs::cxs::encode(&PUBKEY_ACCOUNT).expect("encode separator");
        assert_eq!(encoded.as_slice(), PUBKEY_ACCOUNT.as_bytes());
    }
}
