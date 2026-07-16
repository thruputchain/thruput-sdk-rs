//! The compile-time [`DomainSeparator`] type and the [`domain_separator!`] macro.

use xdrs::{ConstWriteXDR, WriteXDR};

/// A compile-time domain separator: the CXS bytes of
/// `DomainSeparator { ext: 0, domain_separator: <name> }`, held in rodata.
///
/// Construct one with [`crate::domain_separator!`].
pub struct DomainSeparator {
    cxs: &'static [u8],
}

impl DomainSeparator {
    /// Wraps precomputed CXS prefix bytes. Called by [`domain_separator!`];
    /// not intended for direct use.
    #[doc(hidden)]
    pub const fn from_cxs_bytes(cxs: &'static [u8]) -> Self {
        Self { cxs }
    }

    /// The CXS-encoded domain-separator prefix bytes.
    pub const fn as_bytes(&self) -> &'static [u8] {
        self.cxs
    }
}

/// Mirror of `groundhog_xdr::preimage::DomainSeparator` whose opaque field is a
/// borrow (`&[u8]`) rather than `BytesM<256>`, so a value can be built in a
/// `const` (the real type's `BytesM<256>` is heap-backed under `alloc`). It
/// encodes byte-identically; [`DomainSeparatorRepr::to_domain_separator`] and a
/// test pin it to the real type.
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
    #[cfg(feature = "alloc")]
    #[doc(hidden)]
    pub fn to_domain_separator(
        &self,
    ) -> groundhog_xdr::preimage::DomainSeparator {
        groundhog_xdr::preimage::DomainSeparator {
            ext: groundhog_xdr::types::ExtensionPoint::_0,
            domain_separator: xdrs::BytesM::<256>::try_from(self.name)
                .expect("domain separator name exceeds BytesM<256>"),
        }
    }
}

/// Computes the CXS-encoded size of a [`DomainSeparatorRepr`]. Macro support.
#[doc(hidden)]
pub const fn __prefix_size(repr: &DomainSeparatorRepr) -> usize {
    repr.const_cxs_size()
}

/// CXS-encodes a [`DomainSeparatorRepr`] into a fixed `[u8; N]`. Macro support.
#[doc(hidden)]
pub const fn __prefix_bytes<const N: usize>(
    repr: &DomainSeparatorRepr,
) -> [u8; N] {
    let mut w = xdrs::cxs::ConstWriter::<N>::new();
    repr.const_write_cxs(&mut w);
    w.into_bytes()
}

/// Declare a [`DomainSeparator`] from a name literal. The CXS prefix bytes are
/// computed at compile time and stored in rodata.
///
/// ```
/// use groundhog_domain_separator::{domain_separator, DomainSeparator};
/// const MY_SEP: DomainSeparator = domain_separator!("MyThing");
/// ```
#[macro_export]
macro_rules! domain_separator {
    ($name:literal) => {{
        const NAME: &[u8] = $name.as_bytes();
        const _: () = ::core::assert!(
            NAME.len() <= 256,
            "domain separator name exceeds BytesM<256>"
        );
        const REPR: $crate::DomainSeparatorRepr<'static> =
            $crate::DomainSeparatorRepr { ext: 0, name: NAME };
        const N: usize = $crate::__prefix_size(&REPR);
        const BYTES: [u8; N] = $crate::__prefix_bytes::<N>(&REPR);
        $crate::DomainSeparator::from_cxs_bytes(&BYTES)
    }};
}

/// Domain separator for pubkey account address derivation (`"PubkeyAccount"`).
pub const PUBKEY_ACCOUNT: DomainSeparator = domain_separator!("PubkeyAccount");

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
    }
}
