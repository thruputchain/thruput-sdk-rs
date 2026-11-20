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

/// Mirror of `thruput_xdr::preimage::DomainSeparator` whose opaque field is a
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
    ) -> thruput_xdr::preimage::DomainSeparator {
        thruput_xdr::preimage::DomainSeparator {
            ext: thruput_xdr::types::ExtensionPoint::_0,
            domain_separator: xdrs::BytesM::<256>::try_from(self.name)
                .expect("domain separator name exceeds BytesM<256>"),
        }
    }
}

/// CXS-encodes a [`DomainSeparatorRepr`] into a fixed `[u8; N]`.
pub(crate) const fn prefix_bytes<const N: usize>(
    repr: &DomainSeparatorRepr,
) -> [u8; N] {
    let mut w = xdrs::cxs::ConstWriter::<N>::new();
    repr.const_write_cxs(&mut w);
    w.into_bytes()
}

// Each invocation owns a static array sized for its encoded name.
macro_rules! domain_separator {
    ($name:literal) => {{
        const REPR: DomainSeparatorRepr<'static> = DomainSeparatorRepr {
            ext: 0,
            name: $name,
        };
        const _: () = assert!(
            REPR.name.len() <= 256,
            "domain separator name exceeds BytesM<256>"
        );
        const BYTES: [u8; REPR.const_cxs_size()] = prefix_bytes(&REPR);
        DomainSeparator::from_cxs_bytes(&BYTES)
    }};
}

/// Domain separator for pubkey account address derivation (`"PubkeyAccount"`).
pub const PUBKEY_ACCOUNT: DomainSeparator =
    domain_separator!(b"PubkeyAccount");

/// Domain separator for public key hashes (`"PublicKey"`): a `PublicKeyHash`
/// is `sha256( cxs(DomainSeparator("PublicKey")) || cxs(PublicKey) )`, so it
/// commits to the key's scheme as well as its bytes.
pub const PUBLIC_KEY: DomainSeparator = domain_separator!(b"PublicKey");

/// Domain separator for transaction signature preimages (`"Transaction"`).
///
/// Unlike [`PUBKEY_ACCOUNT`] this one prefixes a message that is signed
/// directly rather than hashed; see `transaction_signature_preimage`.
pub const TRANSACTION: DomainSeparator = domain_separator!(b"Transaction");

/// Domain separator for signed operations (`"SignedOp"`); see `SignedOp`.
///
/// Signed directly, like [`TRANSACTION`]. Not a protocol separator: the
/// validator never sees these bytes, only contracts and clients do. If the
/// envelope layout ever changes, give the new layout a new name so signatures
/// made under the old one stop verifying.
pub const SIGNED_OP: DomainSeparator = domain_separator!(b"SignedOp");

/// Domain separator for the transaction proposal hash in a consensus value.
pub const PROPOSAL_TXS: DomainSeparator = domain_separator!(b"ProposalTxs");

/// Domain separator for each parallel batch hash in a transaction proposal.
pub const PARALLEL_BATCH: DomainSeparator =
    domain_separator!(b"ParallelBatch");

#[cfg(all(test, feature = "alloc", feature = "sha2"))]
mod tests {
    use super::*;

    #[test]
    fn prefix_matches_real_domain_separator() {
        for (separator, name) in [
            (&PUBKEY_ACCOUNT, b"PubkeyAccount".as_slice()),
            (&PROPOSAL_TXS, b"ProposalTxs".as_slice()),
            (&PARALLEL_BATCH, b"ParallelBatch".as_slice()),
        ] {
            let repr = DomainSeparatorRepr { ext: 0, name };
            let encoded = xdrs::cxs::encode(&repr.to_domain_separator())
                .expect("encode DomainSeparator");
            assert_eq!(separator.as_bytes(), encoded.as_slice());
        }
    }

    #[test]
    fn prefix_expected_bytes() {
        // 0x00 (ext discriminant) + 0x0d (opaque len = 13) + "PubkeyAccount".
        assert_eq!(PUBKEY_ACCOUNT.as_bytes(), &b"\x00\x0dPubkeyAccount"[..]);
        // 0x00 (ext discriminant) + 0x0b (opaque len = 11) + "Transaction".
        assert_eq!(TRANSACTION.as_bytes(), &b"\x00\x0bTransaction"[..]);
        // 0x00 (ext discriminant) + 0x09 (opaque len = 9) + "PublicKey".
        assert_eq!(PUBLIC_KEY.as_bytes(), &b"\x00\x09PublicKey"[..]);
    }

    #[test]
    fn separators_are_distinct() {
        let all = [
            &PUBKEY_ACCOUNT,
            &PUBLIC_KEY,
            &TRANSACTION,
            &SIGNED_OP,
            &PROPOSAL_TXS,
            &PARALLEL_BATCH,
        ];
        for (i, a) in all.iter().enumerate() {
            for b in &all[i + 1..] {
                assert_ne!(a.as_bytes(), b.as_bytes());
            }
        }
    }

    #[test]
    fn signed_op_expected_bytes() {
        // 0x00 (ext discriminant) + 0x08 (opaque len) + "SignedOp".
        assert_eq!(SIGNED_OP.as_bytes(), &b"\x00\x08SignedOp"[..]);
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
