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
    /// # Panics
    ///
    /// Panics if `scratch` is smaller than
    /// `self.as_bytes().len() + cxs_size(object)`.
    pub fn hash_into<T: WriteXDR + ?Sized>(
        &self,
        object: &T,
        scratch: &mut [MaybeUninit<u8>],
    ) -> SHA256Hash {
        let preimage = HashPreimage {
            domain_separator: self,
            object,
        };
        let total = xdrs::cxs::encode_into(&preimage, scratch)
            .expect("domain-separator scratch too small for the preimage");
        // SAFETY: `encode_into` returned `Ok(total)`. It writes through
        // `IntoBufWriter`, which advances its counter only via a private
        // `reserve` whose callers (`put_byte`/`put_bytes`) fill the entire
        // reserved region immediately, so `scratch[..total]` is initialized.
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
        assert_eq!(PUBKEY_ACCOUNT.hash_into(&obj, &mut scratch), expected);
    }

    #[test]
    fn hash_into_matches_hash() {
        let obj: u64 = 12345;
        let mut scratch = [MaybeUninit::<u8>::uninit(); 64];
        assert_eq!(
            PUBKEY_ACCOUNT.hash_into(&obj, &mut scratch),
            PUBKEY_ACCOUNT.hash(&obj)
        );
    }
}
