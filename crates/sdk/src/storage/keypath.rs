use crate::crypto::sha256;
use crate::types::StorageKey;
use core::fmt::Debug;
use xdrs::WriteXDR;

/// Append-only byte buffer backing a [`Keypath`] preimage.
///
/// Two implementations exist: a growing heap `Vec<u8>` (with the `alloc`
/// feature) and the fixed [`StaticKeyBuf`] fallback (without). Both append
/// identical bytes, so derived storage keys never depend on the feature —
/// only memory behavior and the overflow failure mode differ.
pub trait KeyBuf: Default + Clone + Debug {
    /// Append `bytes`, throwing `LibraryExn::StorageKeypathOverflow` if the
    /// buffer cannot hold them.
    fn append_bytes(&mut self, bytes: &[u8]);
    fn as_slice(&self) -> &[u8];
}

/// Fixed-capacity [`KeyBuf`] used when the `alloc` feature is disabled.
#[derive(Clone, Debug)]
pub struct StaticKeyBuf<const N: usize> {
    buf: [u8; N],
    len: u16,
}

impl<const N: usize> Default for StaticKeyBuf<N> {
    fn default() -> Self {
        Self {
            buf: [0; N],
            len: 0,
        }
    }
}

impl<const N: usize> KeyBuf for StaticKeyBuf<N> {
    fn append_bytes(&mut self, bytes: &[u8]) {
        const { assert!(N <= u16::MAX as usize) };
        let start = self.len as usize;
        let end = start.saturating_add(bytes.len());
        if end > N {
            crate::exn::library_throw(
                crate::exn::LibraryExn::StorageKeypathOverflow,
            );
        }
        self.buf[start..end].copy_from_slice(bytes);
        self.len = end as u16;
    }

    fn as_slice(&self) -> &[u8] {
        &self.buf[..self.len as usize]
    }
}

#[cfg(feature = "alloc")]
impl KeyBuf for alloc::vec::Vec<u8> {
    fn append_bytes(&mut self, bytes: &[u8]) {
        self.extend_from_slice(bytes);
    }

    fn as_slice(&self) -> &[u8] {
        self
    }
}

/// No-alloc keypath preimage budget (total framed bytes across all levels).
pub const STATIC_KEYPATH_CAPACITY: usize = 256;

#[cfg(feature = "alloc")]
type KeyBufImpl = alloc::vec::Vec<u8>;
#[cfg(not(feature = "alloc"))]
type KeyBufImpl = StaticKeyBuf<STATIC_KEYPATH_CAPACITY>;

const TAG_ITEM: u8 = 0x00;
const TAG_INDEX: u8 = 0x01;

/// Path identifying a storage slot.
///
/// A keypath is a sequence of framed components appended to a byte buffer:
/// [`child_index`](Self::child_index) appends `[0x01] ‖ i.to_be_bytes()`
/// (SDK-assigned structural positions: struct fields, Vector len/values
/// regions) and [`child_item`](Self::child_item) appends
/// `[0x00] ‖ u16-len ‖ encode(t)` (user keys, e.g. Map entries). The
/// storage key is `sha256` of the buffer, computed once and memoized by
/// [`KeyCache`]. Framing makes distinct component sequences produce
/// distinct preimages.
///
/// Without the `alloc` feature, the preimage shares a fixed
/// [`STATIC_KEYPATH_CAPACITY`]-byte budget; exceeding it throws
/// `LibraryExn::StorageKeypathOverflow` (enable `alloc`, or pre-hash
/// oversized keys). A single `child_item` encoding is additionally capped
/// at the SDK's fixed encode-buffer size in both modes.
#[derive(Clone, Debug, Default)]
pub struct Keypath {
    buf: KeyBufImpl,
    exclusive: bool,
}

impl Keypath {
    pub fn root() -> Self {
        Self::default()
    }

    /// Child at an SDK-assigned structural position.
    pub fn child_index(&self, i: u64) -> Keypath {
        let mut child = self.clone();
        child.buf.append_bytes(&[TAG_INDEX]);
        child.buf.append_bytes(&i.to_be_bytes());
        child
    }

    /// Child keyed by an encodable value (e.g. a Map key).
    ///
    /// A single item's encoding is capped at the SDK's fixed encode-buffer
    /// size (~1 KiB) in both modes (with and without `alloc`); oversized
    /// items throw `LibraryExn::StorageKeypathOverflow`.
    pub fn child_item<I: WriteXDR + ?Sized>(&self, item: &I) -> Keypath {
        let mut child = self.clone();
        // Encode the component into the reused scratch in place (no fresh
        // stack buffer), then frame it into the keypath's own buffer.
        crate::call::with_scratch(|buf| {
            crate::internal::encode_scratch(buf, item).unwrap_or_else(|_| {
                crate::exn::library_throw(
                    crate::exn::LibraryExn::StorageSerialization,
                )
            });
            let bytes: &[u8] = buf.as_slice();
            let len: u16 = bytes.len().try_into().unwrap_or_else(|_| {
                crate::exn::library_throw(
                    crate::exn::LibraryExn::StorageKeypathOverflow,
                )
            });
            child.buf.append_bytes(&[TAG_ITEM]);
            child.buf.append_bytes(&len.to_be_bytes());
            child.buf.append_bytes(bytes);
        });
        child
    }

    pub fn generate_key(&self) -> StorageKey {
        sha256(self.buf.as_slice()).into()
    }

    pub fn is_exclusive(&self) -> bool {
        self.exclusive
    }

    pub fn set_exclusive(&mut self) {
        self.exclusive = true;
    }
}

#[cfg(test)]
mod test_keypath {
    use super::*;

    #[test]
    fn exclusive_flag_propagates_through_clone() {
        let mut kp = Keypath::root().child_item("some bytes");
        kp.set_exclusive();
        assert!(kp.clone().is_exclusive());
    }

    #[test]
    fn distinct_paths_distinct_keys() {
        let root = Keypath::root();
        // boundary-shift case: [AA BB]+[CC] vs [AA]+[BB CC]
        // Framing (tag + length prefix) makes these distinct preimages.
        let a = root.child_item(&[0xAAu8, 0xBB]).child_item(&[0xCCu8]);
        let b = root.child_item(&[0xAAu8]).child_item(&[0xBBu8, 0xCC]);
        assert_ne!(a.buf.as_slice(), b.buf.as_slice());
        // index vs same-value u64 item: different tags (0x01 vs 0x00)
        let c = root.child_index(0);
        let d = root.child_item(&0u64);
        assert_ne!(c.buf.as_slice(), d.buf.as_slice());
    }

    #[test]
    fn derivation_is_deterministic() {
        let k1 = Keypath::root().child_index(3).child_item(&42u64);
        let k2 = Keypath::root().child_index(3).child_item(&42u64);
        assert_eq!(k1.buf.as_slice(), k2.buf.as_slice());
    }

    #[test]
    fn static_buf_fills_to_exact_capacity() {
        let mut s = StaticKeyBuf::<4>::default();
        s.append_bytes(&[1, 2]);
        s.append_bytes(&[3, 4]); // exactly N — must succeed
        assert_eq!(s.as_slice(), &[1, 2, 3, 4]);
    }

    #[test]
    #[should_panic]
    fn static_buf_overflow_throws() {
        let mut s = StaticKeyBuf::<4>::default();
        s.append_bytes(&[1, 2, 3, 4, 5]);
    }

    #[test]
    fn golden_key_vector() {
        // Preimage: TAG_INDEX(0x01) ‖ 1u64 BE ‖ TAG_ITEM(0x00) ‖ 0001 u16 BE ‖ 00 (0u32 CXS)
        // Note: item encoding uses CXS (Compact XDR Serialization), so 0u32 encodes to 1 byte.
        let kp = Keypath::root().child_index(1).child_item(&0u32);
        let preimage = kp.buf.as_slice();
        let expected_preimage: &[u8] = &[
            0x01, // TAG_INDEX
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, // 1u64 BE
            0x00, // TAG_ITEM
            0x00, 0x01, // len=1 u16 BE (CXS encodes 0u32 as 1 byte)
            0x00, // 0u32 CXS-encoded
        ];
        assert_eq!(
            preimage, expected_preimage,
            "preimage framing changed — update the golden key too"
        );
        // Compute sha256 directly (avoids the syscall mock setup requirement).
        // sha256([01 00 00 00 00 00 00 00 01  00  00 01  00])
        use sha2::Digest;
        let digest: [u8; 32] = sha2::Sha256::digest(preimage).into();
        let expected = hex_literal::hex!(
            "992ef5938f42a0a604538ac271e506422da30b1469f3a480f92c02c2705440e8"
        );
        assert_eq!(digest, expected, "golden key changed");
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn static_and_vec_bufs_agree() {
        fn fill<B: KeyBuf>(buf: &mut B) {
            buf.append_bytes(&[1, 2, 3]);
            buf.append_bytes(&[4]);
        }
        let mut s = StaticKeyBuf::<64>::default();
        let mut v = alloc::vec::Vec::<u8>::default();
        fill(&mut s);
        fill(&mut v);
        assert_eq!(s.as_slice(), v.as_slice());
    }
}

pub use keycache::*;
mod keycache {
    use super::Keypath;
    use crate::types::StorageKey;
    use core::cell::OnceCell;
    use core::ops::Deref;

    #[derive(Debug)]
    pub struct KeyCache {
        keypath: Keypath,
        key: OnceCell<StorageKey>,
    }

    impl From<Keypath> for KeyCache {
        fn from(keypath: Keypath) -> Self {
            KeyCache {
                keypath,
                key: OnceCell::new(),
            }
        }
    }

    impl KeyCache {
        pub fn get(&self) -> &StorageKey {
            self.key.get_or_init(|| self.keypath.generate_key())
        }

        pub fn keypath(&self) -> &Keypath {
            &self.keypath
        }
    }

    impl Deref for KeyCache {
        type Target = StorageKey;

        fn deref(&self) -> &Self::Target {
            self.get()
        }
    }
}
