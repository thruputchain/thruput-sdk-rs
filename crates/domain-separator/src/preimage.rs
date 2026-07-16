//! Domain-separated hashing: `sha256( domain_separator || cxs(object) )`.

use core::mem::MaybeUninit;

use groundhog_xdr::types::SHA256Hash;
use xdrs::WriteXDR;

use crate::DomainSeparator;
use crate::backend::sha256_raw;

/// The semantic preimage `domain_separator || object` — the Rust stand-in for
/// the XDR `VersionedPreimage<T>` that generics can't express. Internal.
struct HashPreimage<'a, T: WriteXDR + ?Sized> {
    domain_separator: &'a DomainSeparator,
    object: &'a T,
}

impl<T: WriteXDR + ?Sized> HashPreimage<'_, T> {
    /// Serialize `prefix || cxs(object)` into `buf`, returning the total length.
    /// Panics if `buf` is too small.
    fn write_into(&self, buf: &mut [MaybeUninit<u8>]) -> usize {
        let prefix = self.domain_separator.as_bytes();
        assert!(
            buf.len() >= prefix.len(),
            "domain-separator scratch too small for the prefix"
        );
        for (slot, &byte) in buf.iter_mut().zip(prefix) {
            slot.write(byte);
        }
        let written =
            xdrs::cxs::encode_into(self.object, &mut buf[prefix.len()..])
                .expect("domain-separator scratch too small for the object");
        prefix.len() + written
    }
}

impl DomainSeparator {
    /// `sha256(self || cxs(object))`, serializing `object` through `scratch`.
    ///
    /// `scratch` must hold at least `self.as_bytes().len() + cxs_size(object)`
    /// bytes; a too-small buffer panics.
    pub fn hash_into<T: WriteXDR + ?Sized>(
        &self,
        object: &T,
        scratch: &mut [MaybeUninit<u8>],
    ) -> SHA256Hash {
        let preimage = HashPreimage {
            domain_separator: self,
            object,
        };
        let total = preimage.write_into(scratch);
        // SAFETY: `write_into` initializes `scratch[..total]`. The prefix loop
        // calls `slot.write(byte)` for every byte in `0..prefix.len()`, and
        // `xdrs::cxs::encode_into` uses `IntoBufWriter` which calls
        // `MaybeUninit::write` / `write_copy_of_slice` for every byte it
        // advances its counter by — so `scratch[prefix.len()..total]` is also
        // fully initialized.
        let bytes = unsafe {
            core::slice::from_raw_parts(scratch.as_ptr() as *const u8, total)
        };
        SHA256Hash::new(sha256_raw(bytes))
    }

    /// `sha256(self || cxs(object))`, allocating the scratch buffer.
    #[cfg(feature = "alloc")]
    #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
    pub fn hash<T: WriteXDR + ?Sized>(&self, object: &T) -> SHA256Hash {
        let object_cxs = xdrs::cxs::encode(object).expect("cxs encode failed");
        let prefix = self.as_bytes();
        let mut buf =
            alloc::vec::Vec::with_capacity(prefix.len() + object_cxs.len());
        buf.extend_from_slice(prefix);
        buf.extend_from_slice(&object_cxs);
        SHA256Hash::new(sha256_raw(&buf))
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;

    const TEST: DomainSeparator = crate::domain_separator!("Test");

    fn manual(sep: &DomainSeparator, object_cxs: &[u8]) -> SHA256Hash {
        let mut buf = sep.as_bytes().to_vec();
        buf.extend_from_slice(object_cxs);
        SHA256Hash::new(sha256_raw(&buf))
    }

    #[test]
    fn hash_matches_manual_concatenation() {
        let obj: u64 = 0xdead_beef;
        let expected = manual(&TEST, &xdrs::cxs::encode(&obj).unwrap());
        assert_eq!(TEST.hash(&obj), expected);
    }

    #[test]
    fn hash_into_matches_hash() {
        let obj: u64 = 12345;
        let mut scratch = [MaybeUninit::<u8>::uninit(); 64];
        assert_eq!(TEST.hash_into(&obj, &mut scratch), TEST.hash(&obj));
    }
}
