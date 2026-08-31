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
    ///
    /// Hidden implementation detail of [`domain_separator!`]: the bytes must be
    /// a faithful CXS encoding of a [`DomainSeparatorRepr`], which only the
    /// macro guarantees. Declare separators with the macro, not this.
    #[doc(hidden)]
    pub const fn from_cxs_bytes(cxs: &'static [u8]) -> Self {
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
/// Hidden: this exists only to compute prefix constants for
/// [`domain_separator!`], which expands at its call site and so needs to name
/// it.
#[doc(hidden)]
#[derive(ConstWriteXDR, WriteXDR)]
pub struct DomainSeparatorRepr<'a> {
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
#[doc(hidden)]
pub const fn prefix_size(repr: &DomainSeparatorRepr) -> usize {
    repr.const_cxs_size()
}

/// CXS-encodes a [`DomainSeparatorRepr`] into a fixed `[u8; N]`.
#[doc(hidden)]
pub const fn prefix_bytes<const N: usize>(
    repr: &DomainSeparatorRepr,
) -> [u8; N] {
    let mut w = xdrs::cxs::ConstWriter::<N>::new();
    repr.const_write_cxs(&mut w);
    w.into_bytes()
}

// --- Declaring a separator -------------------------------------------------

/// Declares a [`DomainSeparator`] constant, computing its CXS prefix at compile
/// time into rodata.
///
/// Every signing domain needs its own separator. Reusing another domain's — in
/// particular [`TRANSACTION`] — is what lets a signature over one object be
/// replayed as a signature over a different object whose encoding the first can
/// be made to imitate.
///
/// ```
/// use groundhog_domain_separator::domain_separator;
///
/// domain_separator! {
///     /// Separator for MyApp's signed operations.
///     pub const MY_APP = "MyApp" version "1";
/// }
///
/// // The version is folded into the protocol's `opaque<256>` name field.
/// assert_eq!(MY_APP.as_bytes(), b"\x00\x08MyApp::1");
/// ```
///
/// # The version
///
/// `version` is the schema version of the messages signed under this separator,
/// and it exists to be bumped: `"MyApp::1"` and `"MyApp::2"` are different
/// domains, so bumping invalidates every signature made under the old schema at
/// once. Bump it whenever a signed message's shape changes — adding a field to
/// a struct does not change its name, and CXS has no framing to notice with.
///
/// It rides inside the name rather than as its own field because
/// `DomainSeparator` is a protocol type (`groundhog_xdr::preimage`) shared with
/// the C++ validator: a new field would change the encoding of *every*
/// separator, moving every pubkey account address and invalidating every
/// in-flight transaction signature. The `::` is the same separator, for the
/// same reason, as the one in entrypoint IDs and event signatures — without it
/// `("MyApp1", "2")` and `("MyApp", "12")` would collide.
///
/// # The unversioned form
///
/// ```ignore
/// domain_separator! {
///     pub const TRANSACTION = unversioned "Transaction";
/// }
/// ```
///
/// This is for the handful of separators whose names the protocol fixes and the
/// validator hardcodes. Contract code wants the versioned form; an unversioned
/// app separator has no way to retire its old signatures.
#[macro_export]
macro_rules! domain_separator {
    (
        $(#[$meta:meta])*
        $vis:vis const $name:ident = $app:literal version $version:literal;
    ) => {
        $crate::domain_separator! {
            $(#[$meta])*
            $vis const $name =
                unversioned ::core::concat!($app, "::", $version);
        }
    };

    (
        $(#[$meta:meta])*
        $vis:vis const $name:ident = unversioned $full:expr;
    ) => {
        $(#[$meta])*
        $vis const $name: $crate::DomainSeparator = {
            const NAME: &[u8] = ($full).as_bytes();

            const _: () = ::core::assert!(
                NAME.len() <= 256,
                "domain separator name exceeds the protocol's opaque<256> bound",
            );

            const REPR: $crate::DomainSeparatorRepr<'static> =
                $crate::DomainSeparatorRepr {
                    ext: 0,
                    name: NAME,
                };
            const N: usize = $crate::prefix_size(&REPR);
            const BYTES: [u8; N] = $crate::prefix_bytes::<N>(&REPR);

            $crate::DomainSeparator::from_cxs_bytes(&BYTES)
        };
    };
}

// --- Protocol separators ----------------------------------------------------
//
// These two names are fixed by the protocol and hardcoded in the C++ validator,
// so they take the unversioned form: their schema version is the protocol's,
// not an application's, and neither can be retired independently.

domain_separator! {
    /// Domain separator for pubkey account address derivation
    /// (`"PubkeyAccount"`).
    pub const PUBKEY_ACCOUNT = unversioned "PubkeyAccount";
}

domain_separator! {
    /// Domain separator for transaction signature preimages (`"Transaction"`).
    ///
    /// Unlike [`PUBKEY_ACCOUNT`] this one prefixes a message that is signed
    /// directly rather than hashed; see `transaction_signature_preimage`.
    pub const TRANSACTION = unversioned "Transaction";
}

domain_separator! {
    /// Domain separator for signed operations (`"SignedOp::1"`).
    ///
    /// Operations share this one separator; what tells them apart inside the
    /// preimage is the payload type's own definition. See [`SignedOp`].
    ///
    /// [`SignedOp`]: crate::SignedOp
    pub const SIGNED_OP = "SignedOp" version "1";
}

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
        assert_ne!(PUBKEY_ACCOUNT.as_bytes(), crate::SIGNED_OP.as_bytes());
        assert_ne!(TRANSACTION.as_bytes(), crate::SIGNED_OP.as_bytes());
    }

    #[test]
    fn signed_op_expected_bytes() {
        // 0x00 (ext discriminant) + 0x0b (opaque len = 11) + "SignedOp::1".
        assert_eq!(crate::SIGNED_OP.as_bytes(), &b"\x00\x0bSignedOp::1"[..]);
    }

    domain_separator! {
        const V1 = "MyApp" version "1";
    }

    domain_separator! {
        const V2 = "MyApp" version "2";
    }

    #[test]
    fn version_is_folded_into_the_name() {
        assert_eq!(V1.as_bytes(), &b"\x00\x08MyApp::1"[..]);
    }

    /// The reason the field exists: bumping it retires every signature made
    /// under the old schema, because the prefix every preimage carries changes.
    #[test]
    fn bumping_the_version_changes_the_domain() {
        assert_ne!(V1.as_bytes(), V2.as_bytes());
    }

    /// `::` is load-bearing. Without it `("MyApp1", "2")` and `("MyApp", "12")`
    /// would both encode as `MyApp12` and share a signing domain — the same
    /// collision `contract_entrypoint_id` guards against.
    #[test]
    fn version_boundary_is_unambiguous() {
        domain_separator! {
            const A = "MyApp1" version "2";
        }
        domain_separator! {
            const B = "MyApp" version "12";
        }
        assert_ne!(A.as_bytes(), B.as_bytes());
    }

    /// A macro-declared separator has to be a real `DomainSeparator`, not just
    /// a lookalike: it must hash through the same preimage path.
    #[test]
    fn macro_separator_hashes_like_a_builtin() {
        use crate::backend::Sha2;
        let obj: u64 = 7;
        assert_ne!(V1.hash::<Sha2, _>(&obj), V2.hash::<Sha2, _>(&obj));
        assert_ne!(V1.hash::<Sha2, _>(&obj), PUBKEY_ACCOUNT.hash::<Sha2, _>(&obj));
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
