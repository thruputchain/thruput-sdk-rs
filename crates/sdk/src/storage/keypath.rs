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

/// Path identifying a storage slot.
///
/// A keypath is a sequence of length-framed components appended to a byte
/// buffer. Each call to [`child`](Self::child) appends
/// `CXS(byte_len) ‖ CXS(value)`: the component value CXS-encoded, prefixed by
/// the CXS-encoded byte length of that encoding. The storage key is `sha256`
/// of the buffer, computed once and memoized by [`KeyCache`].
///
/// The self-delimiting `CXS(len)` prefix makes the preimage
/// `child(v1) ‖ child(v2) ‖ …` uniquely decodable: read the length, then read
/// exactly that many value bytes. This holds for *any* value encoding —
/// including fixed opaque `[u8; N]` keys that do not self-delimit — so
/// distinct component sequences can never share a preimage.
///
/// Without the `alloc` feature, the preimage shares a fixed
/// [`STATIC_KEYPATH_CAPACITY`]-byte budget; exceeding it throws
/// `LibraryExn::StorageKeypathOverflow` (enable `alloc`, or pre-hash
/// oversized keys). A single component's encoding is additionally capped at
/// the SDK's fixed encode-buffer size in both modes.
#[derive(Clone, Debug, Default)]
pub struct Keypath {
    buf: KeyBufImpl,
    exclusive: bool,
}

impl Keypath {
    #[doc(hidden)]
    pub fn root() -> Self {
        Self::default()
    }

    /// Child keyed by an encodable value.
    ///
    /// Appends `CXS(byte_len) ‖ CXS(value)`, where `CXS(value)` is the value's
    /// CXS encoding and `byte_len` is its byte length, also CXS-encoded (the
    /// same compact codec, so small lengths cost a single byte). The
    /// self-delimiting length prefix keeps the preimage uniquely decodable for
    /// any value bytes.
    ///
    /// A single value's encoding is capped at the SDK's fixed encode-buffer
    /// size in both modes (with and without `alloc`); oversized values throw
    /// `LibraryExn::StorageKeypathOverflow`.
    pub fn child<I: WriteXDR + ?Sized>(&self, value: &I) -> Keypath {
        let mut child = self.clone();
        // Encode the component into the reused scratch in place (no fresh
        // stack buffer), then frame it into the keypath's own buffer.
        crate::call::with_scratch(|buf| {
            crate::internal::encode_scratch(buf, value).unwrap_or_else(|_| {
                crate::exn::library_throw(
                    crate::exn::LibraryExn::StorageSerialization,
                )
            });
            let bytes: &[u8] = buf.as_slice();
            // CXS-encode the byte length through the SAME compact codec the
            // value uses (small lengths cost a single byte). Encode it into a
            // separate stack buffer so we don't disturb `buf`, which still
            // holds the value bytes we're about to append. A `u32` CXS encoding
            // is at most 5 bytes.
            let len: u32 = bytes.len().try_into().unwrap_or_else(|_| {
                crate::exn::library_throw(
                    crate::exn::LibraryExn::StorageKeypathOverflow,
                )
            });
            let mut len_buf = [core::mem::MaybeUninit::<u8>::uninit(); 5];
            let written = xdrs::cxs::encode_into(&len, &mut len_buf)
                .unwrap_or_else(|_| {
                    crate::exn::library_throw(
                        crate::exn::LibraryExn::StorageSerialization,
                    )
                });
            // Safety: `encode_into` initialized exactly `written` bytes.
            let len_bytes: &[u8] =
                unsafe { xdrs::mu::assume_init_ref(&len_buf[..written]) };
            child.buf.append_bytes(len_bytes);
            child.buf.append_bytes(bytes);
        });
        child
    }

    pub fn generate_key(&self) -> StorageKey {
        sha256(self.buf.as_slice()).into()
    }

    /// Whether `self` and `other` address the same storage slot.
    ///
    /// Slot identity is the keypath preimage: two keypaths address the same
    /// slot iff their framed byte buffers are equal (and therefore hash to
    /// the same [`generate_key`](Self::generate_key)). The `exclusive`
    /// access-mode flag is not part of slot identity. Compares the preimages
    /// directly, so it needs no syscall and no `sha256`.
    pub fn addresses_same_slot(&self, other: &Keypath) -> bool {
        self.buf.as_slice() == other.buf.as_slice()
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
        let mut kp = Keypath::root().child("some bytes");
        kp.set_exclusive();
        assert!(kp.clone().is_exclusive());
    }

    #[test]
    fn distinct_paths_distinct_keys() {
        let root = Keypath::root();
        // boundary-shift case: [AA BB]+[CC] vs [AA]+[BB CC].
        // Fixed opaque [u8; N] keys do NOT self-delimit, so without the
        // CXS(len) prefix these two sequences would share the preimage
        // AA BB CC. The length prefix keeps them distinct.
        let a = root.child(&[0xAAu8, 0xBB]).child(&[0xCCu8]);
        let b = root.child(&[0xAAu8]).child(&[0xBBu8, 0xCC]);
        assert_ne!(a.buf.as_slice(), b.buf.as_slice());
        // Concretely: child(&[AA,BB]) = [02 AA BB], child(&[CC]) = [01 CC];
        // child(&[AA]) = [01 AA], child(&[BB,CC]) = [02 BB CC].
        assert_eq!(a.buf.as_slice(), &[0x02, 0xAA, 0xBB, 0x01, 0xCC]);
        assert_eq!(b.buf.as_slice(), &[0x01, 0xAA, 0x02, 0xBB, 0xCC]);
    }

    #[test]
    fn addresses_same_slot_matches_preimage_equality() {
        let root = Keypath::root();
        let a = root.child(&3u32).child(&42u64);
        let same = root.child(&3u32).child(&42u64);
        let different = root.child(&3u32).child(&43u64);
        assert!(a.addresses_same_slot(&same));
        assert!(!a.addresses_same_slot(&different));
    }

    #[test]
    fn addresses_same_slot_ignores_exclusive_flag() {
        // The exclusive flag is an access mode, not part of slot identity:
        // two otherwise-identical keypaths still address the same slot.
        let base = Keypath::root().child(&7u32);
        let mut exclusive = base.clone();
        exclusive.set_exclusive();
        assert!(exclusive.is_exclusive());
        assert!(!base.is_exclusive());
        assert!(base.addresses_same_slot(&exclusive));
        assert!(exclusive.addresses_same_slot(&base));
    }

    #[test]
    fn derivation_is_deterministic() {
        let k1 = Keypath::root().child(&3u32).child(&42u64);
        let k2 = Keypath::root().child(&3u32).child(&42u64);
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
        // Preimage: child(&1u32) ‖ child(&0u32), each = CXS(len) ‖ CXS(value).
        // CXS encodes small uints in 1 byte, so 1u32 -> [01], 0u32 -> [00],
        // each with byte-length 1 -> CXS(1) = [01].
        let kp = Keypath::root().child(&1u32).child(&0u32);
        let preimage = kp.buf.as_slice();
        let expected_preimage: &[u8] = &[
            0x01, // CXS(len=1) for the first component
            0x01, // CXS(1u32)
            0x01, // CXS(len=1) for the second component
            0x00, // CXS(0u32)
        ];
        assert_eq!(
            preimage, expected_preimage,
            "preimage framing changed — update the golden key too"
        );
        // Compute sha256 directly (avoids the syscall mock setup requirement).
        // sha256([01 01 01 00])
        use sha2::Digest;
        let digest: [u8; 32] = sha2::Sha256::digest(preimage).into();
        let expected = hex_literal::hex!(
            "f896c3a5f9841b6e1f0a22bd35a6a1bc5efb28aaa23b66301ec8098ce57cf99a"
        );
        assert_eq!(digest, expected, "golden key changed");
    }

    #[test]
    fn domain_frame_is_distinct_and_length_framed() {
        let root = Keypath::root();
        // Two roots with different domains diverge.
        let a = root.child("Token").child(&0u32);
        let b = root.child("Vault").child(&0u32);
        assert_ne!(a.buf.as_slice(), b.buf.as_slice());
        // The domain is now just the root's first `child` — no special framing.
        // CXS("AB") = [02 41 42] (opaque-variable: CXS(len=2) ‖ "AB"); the
        // outer child wraps it with CXS(len=3) = [03].
        let dom = root.child("AB");
        assert_eq!(dom.buf.as_slice(), &[0x03, 0x02, b'A', b'B']);
        // Golden sha256 of the domain preimage — locks the on-chain layout.
        use sha2::Digest;
        let digest: [u8; 32] = sha2::Sha256::digest(dom.buf.as_slice()).into();
        let expected = hex_literal::hex!(
            "801d3bfb551c298e3b7a2dfef59a178ce0c8d3f41fec0f3d8c308035c678e516"
        );
        assert_eq!(digest, expected, "golden domain key changed");
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
