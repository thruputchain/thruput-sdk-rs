use crate::crypto::sha256;
use crate::types::StorageKey;
use core::fmt::Debug;
use xdrs::{WriteXDR, cxs::ConstWriter};

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
///
/// Invariant (everything unsafe here relies on it): bytes `buf[..len]` are
/// initialized. `append_bytes` only ever writes contiguously starting at
/// `len` and then extends `len`, so the initialized region is always a
/// prefix. The tail `buf[len..]` is uninitialized and must never be read —
/// which is also why `Default` need not (and does not) zero the buffer.
pub struct StaticKeyBuf<const N: usize> {
    buf: [core::mem::MaybeUninit<u8>; N],
    len: u16,
}

impl<const N: usize> Default for StaticKeyBuf<N> {
    fn default() -> Self {
        Self {
            buf: [core::mem::MaybeUninit::uninit(); N],
            len: 0,
        }
    }
}

impl<const N: usize> Clone for StaticKeyBuf<N> {
    fn clone(&self) -> Self {
        // Copy only the initialized prefix (every `child()` clones the
        // parent buffer, so this also drops a full-width copy per level).
        let mut new = Self::default();
        new.append_bytes(self.as_slice());
        new
    }
}

impl<const N: usize> core::fmt::Debug for StaticKeyBuf<N> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("StaticKeyBuf")
            .field("bytes", &self.as_slice())
            .field("len", &self.len)
            .finish()
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
        for (dst, src) in self.buf[start..end].iter_mut().zip(bytes) {
            dst.write(*src);
        }
        self.len = end as u16;
    }

    fn as_slice(&self) -> &[u8] {
        // Safety: init-prefix invariant (see type docs) — `buf[..len]` was
        // written by `append_bytes` before `len` was extended to cover it.
        unsafe { xdrs::mu::assume_init_ref(&self.buf[..self.len as usize]) }
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
    /// Root keypath for a contract state tree, seeded with the state type's
    /// compile-time-framed storage domain. The seed bytes are exactly what
    /// [`child`](Self::child) would append for the domain string, so the
    /// domain remains "the root's first child frame". Called by
    /// [`contract_state_root`](crate::storage::contract_state_root); not
    /// intended for direct use. For a bare, undomained keypath (tests,
    /// hand-rolled raw storage) use `Keypath::default()`.
    #[doc(hidden)]
    pub fn root(domain: FramedDomain) -> Self {
        let mut kp = Self::default();
        kp.buf.append_bytes(domain.as_bytes());
        kp
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
    ///
    /// Slot identity is the sequence of `child` values from the root: there is
    /// no per-component "structural vs. user-key" tag, so two children of the
    /// *same* node built from the same value address the same slot regardless
    /// of intent. SDK-generated code never mixes schemes under one node (a
    /// struct keys children by field position, a `Map` by user key, a `Vector`
    /// by region index then element index one level down), so distinct slots
    /// stay distinct. A hand-written `From<Keypath>` must uphold the same rule:
    /// give each child of a node a distinct `value`.
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

    /// Child keyed by a component pre-framed at compile time.
    ///
    /// Appends `frame`'s bytes verbatim — byte-identical to what
    /// [`child`](Self::child) would append for the same component value,
    /// with no runtime encoding and no scratch-buffer checkout. Called by
    /// `derive(Storage)`-generated field derivation and fixed-slot SDK
    /// containers; not intended for direct use.
    #[doc(hidden)]
    pub fn child_framed(&self, frame: &'static FramedComponent) -> Keypath {
        let mut child = self.clone();
        child.buf.append_bytes(frame.as_bytes());
        child
    }

    pub fn generate_key(&self) -> StorageKey {
        generate_key_from_bytes(self.buf.as_slice())
    }

    /// Raw preimage bytes of this path.
    ///
    /// The sha256 of these bytes is the storage key (see [`generate_key`](Self::generate_key)).
    /// Useful for path-identity comparisons and drift-guard tests; also returned
    /// by [`KeyCache::path_bytes`] for the `Owned` arm.
    pub fn path_bytes(&self) -> &[u8] {
        self.buf.as_slice()
    }

    /// Seed a `Keypath` from raw framed preimage bytes.
    ///
    /// Used by `KeyCache::derive_child` for the `Const` arm and by
    /// `derive(Storage)`-generated `FromConstPath` fallback impls: builds a
    /// fresh `Keypath` containing the const bytes, then delegates to the
    /// existing `child()` logic — no duplication of frame-encoding. Not
    /// intended for direct use.
    #[doc(hidden)]
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut kp = Self::default();
        kp.buf.append_bytes(bytes);
        kp
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

/// Hash raw preimage `bytes` to produce a [`StorageKey`].
///
/// Used by both arms of [`KeyCache::get`] to ensure byte-identical hashing
/// regardless of whether the preimage is stored as `Const` or `Owned`.
pub(crate) fn generate_key_from_bytes(bytes: &[u8]) -> StorageKey {
    sha256(bytes).into()
}

/// Byte length of [`frame_domain`]'s output for `domain`.
///
/// Const mirror of the framing [`Keypath::child`] applies to a `&str`
/// component: `CXS(byte_len) ‖ CXS(domain)`. Kept honest by the
/// `const_frame_matches_runtime_child` test and by [`frame_domain`]'s
/// exact-fill assertion.
pub const fn framed_domain_len(domain: &str) -> usize {
    // 10 = two CXS(u32) length prefixes at their 5-byte maximum. Framed
    // domains share the keypath preimage budget; a domain anywhere near
    // this cap is a bug, so fail const-eval with a readable message.
    assert!(
        domain.len() + 10 <= STATIC_KEYPATH_CAPACITY,
        "storage domain too long; shorten it with #[storage_domain = \"...\"]"
    );
    let mut inner = ConstWriter::<STATIC_KEYPATH_CAPACITY>::new();
    inner.write_opaque_variable(domain.as_bytes());
    let inner_len = inner.pos();
    let mut outer = ConstWriter::<STATIC_KEYPATH_CAPACITY>::new();
    outer.write_u32(inner_len as u32);
    outer.pos() + inner_len
}

/// Compile-time framing of a storage-domain string: the exact bytes
/// [`Keypath::child`] would append for `domain` at runtime.
///
/// `N` must equal [`framed_domain_len`]`(domain)`; `into_bytes` asserts the
/// exact fill at const-eval time, so any mismatch is a compile error rather
/// than a wrong storage key.
pub const fn frame_domain<const N: usize>(domain: &str) -> [u8; N] {
    let mut probe = ConstWriter::<STATIC_KEYPATH_CAPACITY>::new();
    probe.write_opaque_variable(domain.as_bytes());
    let inner_len = probe.pos();
    let mut w = ConstWriter::<N>::new();
    w.write_u32(inner_len as u32);
    w.write_opaque_variable(domain.as_bytes());
    w.into_bytes()
}

/// Capacity of a [`FramedComponent`]: `CXS(byte_len) ‖ CXS(v)` for a `u32`
/// is at most 1 (length prefix — a CXS `u32` is at most 5 bytes, so its
/// length always encodes in one byte) + 5 (value) bytes.
const MAX_FRAMED_U32: usize = 6;

/// A `u32` keypath component pre-framed at compile time: the exact bytes
/// [`Keypath::child`] would append for the same value, built through the
/// same CXS codec.
///
/// The only constructor is [`from_u32`](Self::from_u32), which does the
/// framing itself — malformed or aliased frames are unrepresentable, so
/// [`Keypath::child_framed`] cannot be handed raw bytes.
#[derive(Clone, Copy, Debug)]
pub struct FramedComponent {
    /// Frame bytes at `buf[..len]`; the tail is zero padding.
    buf: [u8; MAX_FRAMED_U32],
    len: u8,
}

impl FramedComponent {
    /// Frame `v` exactly as [`Keypath::child`]`(&v)` would at runtime:
    /// `CXS(byte_len) ‖ CXS(v)`. Kept honest by the
    /// `const_u32_frame_matches_runtime_child` test. Called by
    /// `derive(Storage)`-generated code and fixed-slot SDK containers; not
    /// intended (but safe) for direct use.
    #[doc(hidden)]
    pub const fn from_u32(v: u32) -> Self {
        let mut probe = ConstWriter::<5>::new();
        probe.write_u32(v);
        let inner_len = probe.pos();
        let mut w = ConstWriter::<MAX_FRAMED_U32>::new();
        w.write_u32(inner_len as u32);
        w.write_u32(v);
        let len = w.pos() as u8;
        // Pad to capacity so `into_bytes`' exact-fill assertion holds;
        // `len` marks the real frame end.
        while w.pos() < MAX_FRAMED_U32 {
            w.write_opaque_fixed(&[0]);
        }
        Self {
            buf: w.into_bytes(),
            len,
        }
    }

    fn as_bytes(&self) -> &[u8] {
        &self.buf[..self.len as usize]
    }

    /// Number of bytes in this frame.
    ///
    /// `const fn` so the length is available at compile time for sizing static
    /// const-path arrays (used by the Task-3 macro and by tests).
    #[doc(hidden)]
    pub const fn len_bytes(&self) -> usize {
        self.len as usize
    }
}

/// A storage domain pre-framed at compile time (see [`frame_domain`]).
///
/// Newtype so a domain-seeded [`Keypath`] root cannot be handed raw,
/// unframed bytes. Constructed only by `derive(Storage)`-generated code
/// (via [`frame_domain`]); not intended for direct use. The domain's name
/// string is not carried — host collision diagnostics identify colliding
/// roots by type path.
#[derive(Clone, Copy, Debug)]
pub struct FramedDomain(&'static [u8]);

impl FramedDomain {
    #[doc(hidden)]
    pub const fn new(framed: &'static [u8]) -> Self {
        Self(framed)
    }

    pub(crate) const fn as_bytes(&self) -> &'static [u8] {
        self.0
    }
}

#[cfg(test)]
mod test_keypath {
    use super::*;

    #[test]
    fn exclusive_flag_propagates_through_clone() {
        let mut kp = Keypath::default().child("some bytes");
        kp.set_exclusive();
        assert!(kp.clone().is_exclusive());
    }

    #[test]
    fn distinct_paths_distinct_keys() {
        let root = Keypath::default();
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
        let root = Keypath::default();
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
        let base = Keypath::default().child(&7u32);
        let mut exclusive = base.clone();
        exclusive.set_exclusive();
        assert!(exclusive.is_exclusive());
        assert!(!base.is_exclusive());
        assert!(base.addresses_same_slot(&exclusive));
        assert!(exclusive.addresses_same_slot(&base));
    }

    #[test]
    fn derivation_is_deterministic() {
        let k1 = Keypath::default().child(&3u32).child(&42u64);
        let k2 = Keypath::default().child(&3u32).child(&42u64);
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
        let kp = Keypath::default().child(&1u32).child(&0u32);
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
        let root = Keypath::default();
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

        // Clone mid-sequence, then keep appending to the clones — static and
        // Vec must still agree at every step.
        let mut s2 = s.clone();
        let mut v2 = v.clone();
        assert_eq!(s2.as_slice(), v2.as_slice());
        s2.append_bytes(&[5, 6]);
        v2.append_bytes(&[5, 6]);
        assert_eq!(s2.as_slice(), v2.as_slice());
        s2.append_bytes(&[7]);
        v2.append_bytes(&[7]);
        assert_eq!(s2.as_slice(), v2.as_slice());
        // Originals must be untouched by the clones' further appends.
        assert_eq!(s.as_slice(), v.as_slice());
        assert_eq!(s.as_slice(), &[1, 2, 3, 4]);
    }

    #[test]
    fn clone_copies_only_initialized_prefix_and_diverges() {
        let mut s = StaticKeyBuf::<8>::default();
        s.append_bytes(&[1, 2, 3]);
        let mut clone = s.clone();
        // Diverge: append DIFFERENT bytes to original and clone.
        s.append_bytes(&[0xAA, 0xBB]);
        clone.append_bytes(&[0xCC]);
        assert_eq!(s.as_slice(), &[1, 2, 3, 0xAA, 0xBB]);
        assert_eq!(clone.as_slice(), &[1, 2, 3, 0xCC]);
    }

    #[test]
    fn clone_of_full_buffer_roundtrips() {
        let mut s = StaticKeyBuf::<4>::default();
        s.append_bytes(&[1, 2, 3, 4]);
        let clone = s.clone();
        assert_eq!(clone.as_slice(), s.as_slice());
        assert_eq!(clone.as_slice(), &[1, 2, 3, 4]);
    }

    #[test]
    fn default_as_slice_is_empty() {
        let s = StaticKeyBuf::<8>::default();
        assert_eq!(s.as_slice(), &[] as &[u8]);
    }

    #[test]
    fn const_u32_frame_matches_runtime_child() {
        // Load-bearing drift guard, same contract as
        // const_frame_matches_runtime_child: const u32 framing must equal
        // the runtime CXS codec byte-for-byte, or const-derived field keys
        // diverge from runtime-derived ones.
        macro_rules! check {
            ($v:expr) => {{
                const FRAMED: FramedComponent = FramedComponent::from_u32($v);
                let runtime = Keypath::default().child(&$v);
                assert_eq!(
                    FRAMED.as_bytes(),
                    runtime.buf.as_slice(),
                    "const u32 framing drifted from runtime child() for {}",
                    $v
                );
            }};
        }
        // Every CXS u32 width boundary: 1-byte header (0..=62), 2-byte
        // (63..=255), 3-byte (256..=65535), 5-byte (65536..).
        check!(0u32);
        check!(1u32);
        check!(62u32);
        check!(63u32);
        check!(255u32);
        check!(256u32);
        check!(65535u32);
        check!(65536u32);
        check!(u32::MAX);

        // Golden bytes: child(&1u32) appends CXS(len=1) ‖ CXS(1) = [01 01]
        // (the same layout golden_key_vector locks for the runtime path).
        const F1: FramedComponent = FramedComponent::from_u32(1u32);
        assert_eq!(F1.as_bytes(), &[0x01, 0x01]);
    }

    #[test]
    fn child_framed_matches_child() {
        const FRAME: FramedComponent = FramedComponent::from_u32(7u32);
        // A non-empty parent (domain + one level) so the append lands on a
        // realistic preimage.
        let parent = Keypath::default().child("Domain").child(&3u32);
        let via_const = parent.child_framed(&FRAME);
        let via_runtime = parent.child(&7u32);
        assert!(via_const.addresses_same_slot(&via_runtime));
        // It is a child, not the parent.
        assert!(!via_const.addresses_same_slot(&parent));
    }

    #[test]
    fn child_framed_preserves_exclusive_flag() {
        const FRAME: FramedComponent = FramedComponent::from_u32(0u32);
        let mut parent = Keypath::default().child("Domain");
        parent.set_exclusive();
        // child_framed clones the parent, so access mode propagates exactly
        // like child() (which also clones).
        assert!(parent.child_framed(&FRAME).is_exclusive());
    }

    #[test]
    fn const_frame_matches_runtime_child() {
        // THE load-bearing test: const framing must equal the runtime CXS
        // codec byte-for-byte, or const-derived storage keys diverge from
        // runtime-derived ones.
        macro_rules! check {
            ($domain:literal) => {{
                const N: usize = framed_domain_len($domain);
                const FRAMED: [u8; N] = frame_domain::<N>($domain);
                let runtime = Keypath::default().child($domain);
                assert_eq!(
                    &FRAMED[..],
                    runtime.buf.as_slice(),
                    "const framing drifted from runtime child() for {:?}",
                    $domain
                );
            }};
        }
        check!("A");
        check!("AB");
        check!("ERC20State");
        check!("a_rather_long_storage_domain_name_for_framing");
        // > 62 bytes: both CXS length prefixes take the 2-byte header form.
        check!(
            "this_domain_is_deliberately_longer_than_sixty_three_bytes_to_exercise_the_two_byte_cxs_length_header"
        );
        check!("日本語ドメイン");

        // Golden bytes for "AB" (same vector as
        // domain_frame_is_distinct_and_length_framed).
        const N_AB: usize = framed_domain_len("AB");
        const AB: [u8; N_AB] = frame_domain::<N_AB>("AB");
        assert_eq!(&AB[..], &[0x03, 0x02, b'A', b'B']);
    }
}

// ── KeyCache unit tests ──────────────────────────────────────────────────────
//
// These tests are the wire-safety anchor for the const-path feature.
// `keycache_const_and_owned_agree` and its long-domain variant are the drift
// guards: they specify exactly which bytes the Task-3 macro must emit and
// confirm that `sha256` of those bytes equals `sha256` of the runtime-derived
// bytes.  If either assertion fails, a const-path handle would address a
// different storage slot than today — a silent data-access bug.
#[cfg(test)]
mod test_keycache {
    use super::*;
    use xdrs::cxs::ConstWriter;

    // ── Shared compile-time path sizes ───────────────────────────────────────

    // "DriftGuard" domain + field-index-0 frame.
    const N_DG: usize = framed_domain_len("DriftGuard");
    // `FramedComponent::from_u32(0)` encodes as CXS(1) ‖ CXS(0) = [0x01, 0x00] (2 bytes).
    const N_F0: usize = FramedComponent::from_u32(0).len_bytes();
    const N_DG_TOTAL: usize = N_DG + N_F0;

    // Long domain (> 62 bytes) + field-index-0 frame.
    const LONG_DOMAIN: &str = "this_domain_is_deliberately_longer_than_sixty_three_bytes_to_exercise_the_two_byte_cxs_length_header";
    const N_LD: usize = framed_domain_len(LONG_DOMAIN);
    const N_LD_TOTAL: usize = N_LD + N_F0;

    // ── Shared static paths ──────────────────────────────────────────────────

    /// Const path for "DriftGuard" domain + field 0.
    ///
    /// SPECIFICATION: This byte sequence is what Task 3's macro MUST emit for a
    /// leaf at field index 0 under a domain with this name. The byte assembly
    /// is `DOMAIN_FRAME ‖ FIELD_FRAME` where both are produced by `ConstWriter`
    /// calls that mirror the runtime `child()` / `child_framed()` codec exactly.
    static DG_F0_PATH: [u8; N_DG_TOTAL] = {
        // Domain frame: identical bytes to `frame_domain::<N_DG>("DriftGuard")`,
        // which equals what `Keypath::child("DriftGuard")` appends at runtime.
        let d: [u8; N_DG] = frame_domain::<N_DG>("DriftGuard");
        // Field-0 frame: CXS(inner_len) ‖ CXS(0u32), mirroring
        // `FramedComponent::from_u32(0)` = `Keypath::child(&0u32)` bytes.
        let f: [u8; N_F0] = {
            let mut probe = ConstWriter::<5>::new();
            probe.write_u32(0u32); // probe how many bytes CXS(0) uses
            let v_len = probe.pos() as u32; // = 1
            let mut fw = ConstWriter::<N_F0>::new();
            fw.write_u32(v_len); // length prefix [0x01]
            fw.write_u32(0u32); // value         [0x00]
            fw.into_bytes() // = [0x01, 0x00]
        };
        let mut w = ConstWriter::<N_DG_TOTAL>::new();
        w.write_opaque_fixed(&d);
        w.write_opaque_fixed(&f);
        w.into_bytes()
    };

    /// Const path for the >62-byte domain + field 0.
    ///
    /// Exercises the CXS 1→2-byte width boundary (the #260 test pattern from
    /// the design spec §5): both the opaque-variable length prefix AND the outer
    /// frame length prefix take the 2-byte CXS form when the domain is longer
    /// than 62 bytes.
    static LD_F0_PATH: [u8; N_LD_TOTAL] = {
        let d: [u8; N_LD] = frame_domain::<N_LD>(
            "this_domain_is_deliberately_longer_than_sixty_three_bytes_to_exercise_the_two_byte_cxs_length_header",
        );
        let f: [u8; N_F0] = {
            let mut probe = ConstWriter::<5>::new();
            probe.write_u32(0u32);
            let v_len = probe.pos() as u32;
            let mut fw = ConstWriter::<N_F0>::new();
            fw.write_u32(v_len);
            fw.write_u32(0u32);
            fw.into_bytes()
        };
        let mut w = ConstWriter::<N_LD_TOTAL>::new();
        w.write_opaque_fixed(&d);
        w.write_opaque_fixed(&f);
        w.into_bytes()
    };

    // ── Drift-guard tests ────────────────────────────────────────────────────

    /// THE load-bearing drift guard for the const-path feature.
    ///
    /// Asserts that `KeyCache::from_const(&DG_F0_PATH)` addresses the same
    /// storage slot as a `KeyCache` built by the runtime derivation path
    /// (`Keypath::root` + `child_framed`). If either assertion fails, a
    /// `Const`-arm handle would silently hash to a different storage key than
    /// today — a hard wire-safety violation.
    ///
    /// Note: `sha256` in SDK tests is a syscall requiring the native-exec host;
    /// we compute it here via the `sha2` crate directly (the same byte-for-byte
    /// algorithm) so the test runs as a pure unit test.
    #[test]
    fn keycache_const_and_owned_agree() {
        use sha2::Digest;
        const D: [u8; N_DG] = frame_domain::<N_DG>("DriftGuard");
        const FIELD_0: FramedComponent = FramedComponent::from_u32(0);
        // Runtime path: identical to what derive(Storage) currently emits.
        let owned =
            Keypath::root(FramedDomain::new(&D)).child_framed(&FIELD_0);
        let owned_cache = KeyCache::from(owned);

        let const_cache = KeyCache::from_const(&DG_F0_PATH);

        // Primary wire-safety assertion: same preimage bytes → same slot.
        assert_eq!(
            const_cache.path_bytes(),
            owned_cache.path_bytes(),
            "Const path bytes differ from runtime-derived bytes — drift detected"
        );
        // Cross-check: sha256 of both preimages agree (same key on-chain).
        let const_hash: [u8; 32] =
            sha2::Sha256::digest(const_cache.path_bytes()).into();
        let owned_hash: [u8; 32] =
            sha2::Sha256::digest(owned_cache.path_bytes()).into();
        assert_eq!(
            const_hash, owned_hash,
            "Const and Owned arms hash to different StorageKeys"
        );
    }

    /// Drift guard for the >62-byte-domain case (CXS 1→2-byte boundary).
    ///
    /// Same contract as `keycache_const_and_owned_agree` but for a domain string
    /// that exceeds 62 bytes, forcing both CXS length prefixes into their 2-byte
    /// form. This pins the multi-byte CXS path that `const_frame_matches_runtime_child`
    /// exercises, now extended to the full KeyCache interface.
    #[test]
    fn keycache_const_and_owned_agree_long_domain() {
        use sha2::Digest;
        const D_L: [u8; N_LD] = frame_domain::<N_LD>(
            "this_domain_is_deliberately_longer_than_sixty_three_bytes_to_exercise_the_two_byte_cxs_length_header",
        );
        const FIELD_0: FramedComponent = FramedComponent::from_u32(0);
        let owned =
            Keypath::root(FramedDomain::new(&D_L)).child_framed(&FIELD_0);
        let owned_cache = KeyCache::from(owned);

        let const_cache = KeyCache::from_const(&LD_F0_PATH);

        assert_eq!(
            const_cache.path_bytes(),
            owned_cache.path_bytes(),
            ">62-byte domain: const path bytes differ from runtime (CXS boundary drift)"
        );
        let const_hash: [u8; 32] =
            sha2::Sha256::digest(const_cache.path_bytes()).into();
        let owned_hash: [u8; 32] =
            sha2::Sha256::digest(owned_cache.path_bytes()).into();
        assert_eq!(
            const_hash, owned_hash,
            ">62-byte domain: Const and Owned arms hash to different StorageKeys"
        );
    }

    // ── Behavioral tests ─────────────────────────────────────────────────────

    /// `path_bytes()` returns stable bytes on repeated calls (no mutation);
    /// the `Const` arm additionally returns the same `'static` pointer.
    ///
    /// Full `get()` pointer-equality (OnceCell memoization) is verified in
    /// integration tests that run with a native-exec host (syscalls_mock feature);
    /// the `OnceCell::get_or_init` contract guarantees it by construction.
    #[test]
    fn keycache_const_hashes_once() {
        use sha2::Digest;
        // Const arm: path_bytes returns the same static slice pointer every time.
        let cache = KeyCache::from_const(&DG_F0_PATH);
        let b1 = cache.path_bytes();
        let b2 = cache.path_bytes();
        assert!(
            core::ptr::eq(b1.as_ptr(), b2.as_ptr()),
            "Const arm path_bytes() returned different pointers"
        );
        // Cross-check sha256 stability via sha2 (no syscall).
        let h1: [u8; 32] = sha2::Sha256::digest(b1).into();
        let h2: [u8; 32] = sha2::Sha256::digest(b2).into();
        assert_eq!(h1, h2);

        // Owned arm: path_bytes returns the same bytes every time.
        const D: [u8; N_DG] = frame_domain::<N_DG>("DriftGuard");
        const F: FramedComponent = FramedComponent::from_u32(0);
        let owned_cache = KeyCache::from(
            Keypath::root(FramedDomain::new(&D)).child_framed(&F),
        );
        assert_eq!(owned_cache.path_bytes(), owned_cache.path_bytes());
    }

    /// `derive_child` from the `Const` arm produces the same child preimage as
    /// from the `Owned` arm — no divergence in dynamic child derivation.
    #[test]
    fn keycache_derive_child_agrees_across_arms() {
        const D: [u8; N_DG] = frame_domain::<N_DG>("DriftGuard");
        const F: FramedComponent = FramedComponent::from_u32(0);
        let owned_kp = Keypath::root(FramedDomain::new(&D)).child_framed(&F);
        let owned_cache = KeyCache::from(owned_kp);
        let const_cache = KeyCache::from_const(&DG_F0_PATH);

        let const_child = const_cache.derive_child(&5u32);
        let owned_child = owned_cache.derive_child(&5u32);
        assert!(
            const_child.addresses_same_slot(&owned_child),
            "derive_child from Const arm differs from Owned arm for key 5u32"
        );

        // Multiple keys, including a large one.
        let const_child_max = const_cache.derive_child(&u32::MAX);
        let owned_child_max = owned_cache.derive_child(&u32::MAX);
        assert!(const_child_max.addresses_same_slot(&owned_child_max));
    }

    /// `is_exclusive()` on the `Const` arm is always `false`; on `Owned` it
    /// reflects the inner `Keypath`'s flag (set before construction by
    /// `Map::at_mut_exclusive`).
    #[test]
    fn keycache_is_exclusive_semantics() {
        // Const arm: never exclusive.
        let const_cache = KeyCache::from_const(&DG_F0_PATH);
        assert!(!const_cache.is_exclusive());

        // Owned arm without flag: not exclusive.
        let kp = Keypath::default().child(&1u32);
        assert!(!KeyCache::from(kp).is_exclusive());

        // Owned arm with flag set before construction.
        let mut kp_excl = Keypath::default().child(&1u32);
        kp_excl.set_exclusive();
        assert!(KeyCache::from(kp_excl).is_exclusive());
    }
}

pub use keycache::*;
mod keycache {
    use super::{Keypath, generate_key_from_bytes};
    use crate::types::StorageKey;
    use core::cell::OnceCell;
    use core::ops::Deref;
    use xdrs::WriteXDR;

    /// Storage for the preimage bytes of a keypath handle.
    ///
    /// - `Const`: a `'static` byte slice assembled at compile time (root-skeleton
    ///   regime). Constructing a `Const` leaf writes only the discriminant + pointer —
    ///   no memcpy of a full-width buffer.
    /// - `Owned`: an owned [`Keypath`] buffer, the current regime for dynamic paths
    ///   (under `Map`/`Vector`, nested subtrees). Identical to today's behavior.
    #[derive(Debug)]
    pub(crate) enum PathRepr {
        Const(&'static [u8]),
        Owned(Keypath),
    }

    /// Per-handle lazy storage key cache.
    ///
    /// Holds the path preimage in either the `Const` (compile-time) or `Owned`
    /// (runtime) representation, and memoizes the `sha256` hash on the first call
    /// to [`get`](Self::get). Hash-on-first-touch behavior is identical for both
    /// arms (preserved from today).
    #[derive(Debug)]
    pub struct KeyCache {
        path: PathRepr,
        key: OnceCell<StorageKey>,
    }

    impl From<Keypath> for KeyCache {
        fn from(keypath: Keypath) -> Self {
            KeyCache {
                path: PathRepr::Owned(keypath),
                key: OnceCell::new(),
            }
        }
    }

    impl KeyCache {
        /// Construct a `KeyCache` from a `'static` byte slice assembled at compile
        /// time. The `sha256` hash is computed lazily on the first call to
        /// [`get`](Self::get).
        ///
        /// `const fn` so this can appear in `const`/`static` initializers.
        pub const fn from_const(path: &'static [u8]) -> Self {
            KeyCache {
                path: PathRepr::Const(path),
                key: OnceCell::new(),
            }
        }

        /// The raw preimage bytes of this handle's storage path.
        ///
        /// - `Const` arm: the `'static` slice supplied to [`Self::from_const`].
        /// - `Owned` arm: the internal byte buffer of the wrapped [`Keypath`].
        ///
        /// `sha256` of this slice is the storage key (see [`get`](Self::get)).
        pub fn path_bytes(&self) -> &[u8] {
            match &self.path {
                PathRepr::Const(bytes) => bytes,
                PathRepr::Owned(kp) => kp.path_bytes(),
            }
        }

        /// Whether this handle carries an exclusive access-mode flag.
        ///
        /// `Const` arm: always `false`. Root-skeleton handles constructed via
        /// [`Self::from_const`] are never map-entry exclusive handles —
        /// `Map::at_mut_exclusive` calls `Keypath::set_exclusive` before
        /// `V::from(keypath)`, so exclusivity only ever appears on `Owned` arms.
        ///
        /// `Owned` arm: propagates the flag from the inner [`Keypath`], preserving
        /// today's observable behavior exactly.
        pub fn is_exclusive(&self) -> bool {
            match &self.path {
                PathRepr::Const(_) => false,
                PathRepr::Owned(kp) => kp.is_exclusive(),
            }
        }

        /// Lazily compute and memoize the [`StorageKey`] (`sha256` of the preimage).
        ///
        /// Hashing behavior is identical to today for both arms: the hash is
        /// computed at most once per handle instance.
        pub fn get(&self) -> &StorageKey {
            self.key
                .get_or_init(|| generate_key_from_bytes(self.path_bytes()))
        }

        /// Derive a child [`Keypath`] for the given dynamic key `value`.
        ///
        /// - `Owned` arm: delegates directly to [`Keypath::child`], unchanged.
        /// - `Const` arm: seeds a fresh [`Keypath`] from the const bytes via
        ///   [`Keypath::from_bytes`], then calls [`Keypath::child`] — no
        ///   duplication of frame-encoding logic.
        ///
        /// Used by `Vector` to derive element keypaths from its `values` map's path.
        pub fn derive_child<I: WriteXDR + ?Sized>(
            &self,
            value: &I,
        ) -> Keypath {
            match &self.path {
                PathRepr::Owned(kp) => kp.child(value),
                PathRepr::Const(bytes) => {
                    Keypath::from_bytes(bytes).child(value)
                }
            }
        }
    }

    impl Deref for KeyCache {
        type Target = StorageKey;

        fn deref(&self) -> &Self::Target {
            self.get()
        }
    }
}
