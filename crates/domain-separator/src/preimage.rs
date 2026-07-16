//! Domain-separated hashing: `sha256( domain_separator || cxs(object) )`.

use core::mem::MaybeUninit;

use groundhog_xdr::types::SHA256Hash;
use xdrs::WriteXDR;

use crate::DomainSeparator;
use crate::backend::sha256_raw;

/// The semantic preimage `domain_separator || object` — the Rust stand-in for
/// the XDR `VersionedPreimage<T>` that XDR schemas can't express generically.
///
/// The layout is spelled once, here: CXS concatenates struct fields with no
/// framing, and `DomainSeparator`'s `WriteXDR` re-emits its precomputed prefix
/// verbatim, so this encodes to exactly `prefix || cxs(object)`. Both
/// [`DomainSeparator::hash`] and [`DomainSeparator::hash_into`] go through it,
/// so the host and guest paths cannot disagree.
///
/// The `?Sized` object field needs `xdrs >= 0.8.2`, where the blanket
/// `impl WriteXDR for &T` accepts unsized `T`.
#[derive(WriteXDR)]
struct HashPreimage<'a, T: WriteXDR + ?Sized> {
    domain_separator: &'a DomainSeparator,
    object: &'a T,
}

impl DomainSeparator {
    /// `sha256(self || cxs(object))`, serializing `object` through `scratch`.
    ///
    /// Takes the unchecked encode path: on gas-metered guests the bounds check
    /// and error path are pure overhead when the buffer is statically sized to
    /// the worst case (see [`crate::pubkey_account_address`]).
    ///
    /// # Safety
    ///
    /// - `scratch` must be at least `self.as_bytes().len()` plus the worst-case
    ///   CXS size of `object`. There is no runtime size check: a short buffer is
    ///   undefined behaviour, not a panic. Size it from
    ///   `<T as xdrs::cxs::ConstMaxSize>::MAX_SIZE` rather than by inspection.
    /// - `object` must respect its declared XDR bounds (e.g. a `VecM<T, MAX>`
    ///   no longer than `MAX`), or the encoding can exceed that size bound.
    pub unsafe fn hash_into<T: WriteXDR + ?Sized>(
        &self,
        object: &T,
        scratch: &mut [MaybeUninit<u8>],
    ) -> SHA256Hash {
        let preimage = HashPreimage {
            domain_separator: self,
            object,
        };
        // SAFETY: forwarded caller contract — `scratch` holds the encoded
        // preimage and `object` respects its declared bounds. The prefix is a
        // fixed opaque, so it adds exactly `as_bytes().len()` bytes.
        let total =
            unsafe { xdrs::cxs::encode_into_unchecked(&preimage, scratch) };
        // SAFETY: the writer initializes every byte it counts, so
        // `scratch[..total]` is initialized.
        let bytes = unsafe {
            core::slice::from_raw_parts(scratch.as_ptr() as *const u8, total)
        };
        SHA256Hash::new(sha256_raw(bytes))
    }

    /// `sha256(self || cxs(object))`, allocating the scratch buffer.
    #[cfg(feature = "alloc")]
    #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
    pub fn hash<T: WriteXDR + ?Sized>(&self, object: &T) -> SHA256Hash {
        let preimage = HashPreimage {
            domain_separator: self,
            object,
        };
        let buf = xdrs::cxs::encode(&preimage).expect("cxs encode failed");
        SHA256Hash::new(sha256_raw(&buf))
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    use crate::PUBKEY_ACCOUNT;

    /// Independently spells the preimage layout as a plain concatenation.
    /// This is the guard on the derived encoding: if `#[derive(WriteXDR)]` on
    /// `HashPreimage` ever emitted framing (a length prefix, padding, a field
    /// count), the derived bytes would stop matching this and the tests below
    /// would fail.
    fn manual(sep: &DomainSeparator, object_cxs: &[u8]) -> SHA256Hash {
        let mut buf = sep.as_bytes().to_vec();
        buf.extend_from_slice(object_cxs);
        SHA256Hash::new(sha256_raw(&buf))
    }

    #[test]
    fn hash_matches_manual_concatenation() {
        let obj: u64 = 0xdead_beef;
        let expected =
            manual(&PUBKEY_ACCOUNT, &xdrs::cxs::encode(&obj).unwrap());
        assert_eq!(PUBKEY_ACCOUNT.hash(&obj), expected);
    }

    #[test]
    fn hash_into_matches_manual_concatenation() {
        let obj: u64 = 0xdead_beef;
        let expected =
            manual(&PUBKEY_ACCOUNT, &xdrs::cxs::encode(&obj).unwrap());
        let mut scratch = [MaybeUninit::<u8>::uninit(); 64];
        assert_eq!(
            unsafe { PUBKEY_ACCOUNT.hash_into(&obj, &mut scratch) },
            expected
        );
    }

    #[test]
    fn hash_into_matches_hash() {
        let obj: u64 = 12345;
        let mut scratch = [MaybeUninit::<u8>::uninit(); 64];
        assert_eq!(
            unsafe { PUBKEY_ACCOUNT.hash_into(&obj, &mut scratch) },
            PUBKEY_ACCOUNT.hash(&obj)
        );
    }
}
