use crate::hasher::Hasher;
use crate::types::StorageKey;
use groundhog_xdr::storage_item::StorageDeltaFlags;
use xdrs::{WriteXDR, cxs::ConstWriter};

/// Path identifying a storage slot.
///
/// A keypath is a sequence of length-framed components absorbed into one of
/// the VM's incremental hash streams. Each call to [`child`](Self::child)
/// appends `CXS(byte_len) ‖ CXS(value)`: the component value CXS-encoded,
/// prefixed by the CXS-encoded byte length of that encoding. The storage key
/// is the SHA-256 of the concatenated frames; leaf storage cells derive it
/// once at construction and hold it as a [`LeafKey`].
///
/// The keypath IS its midstate: it holds the VM stream that has absorbed
/// exactly its frames. [`child`](Self::child) forks the parent's stream — a
/// constant-time `SHA256Copy`, which is what that syscall exists for — and
/// absorbs only its own frame, so a shared parent prefix is absorbed once
/// ever; [`generate_key`](Self::generate_key) forks once more and finalizes.
/// No preimage bytes are kept guest-side.
///
/// The self-delimiting `CXS(len)` prefix makes the preimage
/// `child(v1) ‖ child(v2) ‖ …` uniquely decodable: read the length, then read
/// exactly that many value bytes. This holds for *any* value encoding —
/// including fixed opaque `[u8; N]` keys that do not self-delimit — so
/// distinct component sequences can never share a preimage.
///
/// Like a [`Hasher`], a keypath is meaningful only in the current contract
/// invocation, so it can't be stored. Live keypaths, user hashers, and one
/// transient per key derivation share the VM's live-stream budget (the
/// chain's `max_sha256_buffers` network config parameter); the VM refuses
/// streams past it and the refusing
/// syscall's error is thrown. A stream absorbs at most
/// `kSHA256BufferMaxSize` (4096) preimage bytes, also VM-enforced. A single
/// component's encoding is capped at the SDK's fixed encode-buffer size.
#[derive(Debug, Default)]
pub struct Keypath {
    hasher: Hasher,
    exclusive: bool,
}

impl Clone for Keypath {
    fn clone(&self) -> Self {
        Self {
            hasher: self.hasher.fork(),
            exclusive: self.exclusive,
        }
    }
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
        kp.hasher.update(&domain.as_bytes());
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
    /// size; oversized values throw `LibraryExn::StorageKeypathOverflow`.
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
        // stack buffer), then absorb the frame into the forked stream.
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
                unsafe { len_buf[..written].assume_init_ref() };
            child.hasher.update(&len_bytes);
            child.hasher.update(&bytes);
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
        child.hasher.update(&frame.as_bytes());
        child
    }

    pub fn generate_key(&self) -> StorageKey {
        self.hasher.fork().finish().into()
    }

    /// Consume the keypath and return its storage key, finalizing its own
    /// stream directly — no transient fork.
    pub fn into_key(self) -> StorageKey {
        self.hasher.finish().into()
    }

    /// Whether `self` and `other` address the same storage slot.
    ///
    /// Slot identity is the keypath preimage; SHA-256 is collision
    /// resistant, so equality of the derived keys decides it. The
    /// `exclusive` access-mode flag is not part of slot identity.
    pub fn addresses_same_slot(&self, other: &Keypath) -> bool {
        self.generate_key() == other.generate_key()
    }

    pub fn is_exclusive(&self) -> bool {
        self.exclusive
    }

    pub fn set_exclusive(&mut self) {
        self.exclusive = true;
    }

    /// The flags each write through a handle on this keypath requests after
    /// it succeeds: `ExclusiveLock` for an exclusive keypath. This is the
    /// handle's write policy, not the host's current flags for the key.
    pub fn write_flags(&self) -> StorageDeltaFlags {
        if self.exclusive {
            StorageDeltaFlags::ExclusiveLock
        } else {
            StorageDeltaFlags::None
        }
    }
}

/// Compile-time buffer bound for domain framing (and the domain-length
/// assert in [`framed_domain_len`]): far above any sane domain name, far
/// below the VM's 4096-byte stream input cap.
const DOMAIN_FRAME_MAX: usize = 256;

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
        domain.len() + 10 <= DOMAIN_FRAME_MAX,
        "storage domain too long; shorten it with #[storage_domain = \"...\"]"
    );
    let mut inner = ConstWriter::<DOMAIN_FRAME_MAX>::new();
    inner.write_opaque_variable(domain.as_bytes());
    let inner_len = inner.pos();
    let mut outer = ConstWriter::<DOMAIN_FRAME_MAX>::new();
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
    let mut probe = ConstWriter::<DOMAIN_FRAME_MAX>::new();
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

    /// Key oracle: sha2 over an expected framed preimage, computed
    /// natively with no syscall. With no guest-side preimage bytes to
    /// inspect, drift guards compare stream-derived keys against this.
    fn oracle_key(preimage: &[u8]) -> StorageKey {
        use sha2::Digest;
        let digest: [u8; 32] = sha2::Sha256::digest(preimage).into();
        crate::SHA256Hash::new(digest).into()
    }

    crate::contract_test!(exclusive_flag_propagates_through_clone {
        let mut kp = Keypath::default().child("some bytes");
        kp.set_exclusive();
        assert!(kp.clone().is_exclusive());
    });

    crate::contract_test!(distinct_paths_distinct_keys {
        let root = Keypath::default();
        // boundary-shift case: [AA BB]+[CC] vs [AA]+[BB CC].
        // Fixed opaque [u8; N] keys do NOT self-delimit, so without the
        // CXS(len) prefix these two sequences would share the preimage
        // AA BB CC. The length prefix keeps them distinct.
        let a = root.child(&[0xAAu8, 0xBB]).child(&[0xCCu8]);
        let b = root.child(&[0xAAu8]).child(&[0xBBu8, 0xCC]);
        assert!(!a.addresses_same_slot(&b));
        // Concretely: child(&[AA,BB]) = [02 AA BB], child(&[CC]) = [01 CC];
        // child(&[AA]) = [01 AA], child(&[BB,CC]) = [02 BB CC].
        assert_eq!(
            a.generate_key(),
            oracle_key(&[0x02, 0xAA, 0xBB, 0x01, 0xCC])
        );
        assert_eq!(
            b.generate_key(),
            oracle_key(&[0x01, 0xAA, 0x02, 0xBB, 0xCC])
        );
    });

    crate::contract_test!(clone_copies_state_and_diverges {
        // Differential pin, the deleted buffer tests' role: a clone copies
        // the absorbed state, the original and clone then extend
        // independently, and each key still equals the sha2 of its exact
        // framed preimage — so a divergence on either side shows up.
        let base = Keypath::default().child(&[1u8, 2, 3]);
        let a = base.child(&[0xAAu8, 0xBB]);
        let b = base.child(&[0xCCu8]);
        assert_eq!(
            a.generate_key(),
            oracle_key(&[0x03, 1, 2, 3, 0x02, 0xAA, 0xBB])
        );
        assert_eq!(
            b.generate_key(),
            oracle_key(&[0x03, 1, 2, 3, 0x01, 0xCC])
        );
        // The shared parent is untouched by its children's extensions.
        assert_eq!(base.generate_key(), oracle_key(&[0x03, 1, 2, 3]));
    });

    crate::contract_test!(addresses_same_slot_matches_preimage_equality {
        let root = Keypath::default();
        let a = root.child(&3u32).child(&42u64);
        let same = root.child(&3u32).child(&42u64);
        let different = root.child(&3u32).child(&43u64);
        assert!(a.addresses_same_slot(&same));
        assert!(!a.addresses_same_slot(&different));
    });

    crate::contract_test!(addresses_same_slot_ignores_exclusive_flag {
        // The exclusive flag is an access mode, not part of slot identity:
        // two otherwise-identical keypaths still address the same slot.
        let base = Keypath::default().child(&7u32);
        let mut exclusive = base.clone();
        exclusive.set_exclusive();
        assert!(exclusive.is_exclusive());
        assert!(!base.is_exclusive());
        assert!(base.addresses_same_slot(&exclusive));
        assert!(exclusive.addresses_same_slot(&base));
    });

    crate::contract_test!(derivation_is_deterministic {
        let k1 = Keypath::default().child(&3u32).child(&42u64);
        let k2 = Keypath::default().child(&3u32).child(&42u64);
        assert_eq!(k1.generate_key(), k2.generate_key());
    });

    crate::contract_test!(golden_key_vector {
        // Preimage: child(&1u32) ‖ child(&0u32), each = CXS(len) ‖ CXS(value).
        // CXS encodes small uints in 1 byte, so 1u32 -> [01], 0u32 -> [00],
        // each with byte-length 1 -> CXS(1) = [01]. Locks the on-chain key:
        // sha256([01 01 01 00]).
        let kp = Keypath::default().child(&1u32).child(&0u32);
        let expected: StorageKey = crate::SHA256Hash::new(hex_literal::hex!(
            "f896c3a5f9841b6e1f0a22bd35a6a1bc5efb28aaa23b66301ec8098ce57cf99a"
        ))
        .into();
        assert_eq!(kp.generate_key(), expected, "golden key changed");
    });

    crate::contract_test!(domain_frame_is_distinct_and_length_framed {
        let root = Keypath::default();
        // Two roots with different domains diverge.
        let a = root.child("Token").child(&0u32);
        let b = root.child("Vault").child(&0u32);
        assert!(!a.addresses_same_slot(&b));
        // The domain is just the root's first `child` — no special framing.
        // CXS("AB") = [02 41 42] (opaque-variable: CXS(len=2) ‖ "AB"); the
        // outer child wraps it with CXS(len=3) = [03]. Golden sha256 of the
        // domain preimage locks the on-chain layout.
        let dom = root.child("AB");
        assert_eq!(dom.generate_key(), oracle_key(&[0x03, 0x02, b'A', b'B']));
        let expected: StorageKey = crate::SHA256Hash::new(hex_literal::hex!(
            "801d3bfb551c298e3b7a2dfef59a178ce0c8d3f41fec0f3d8c308035c678e516"
        ))
        .into();
        assert_eq!(dom.generate_key(), expected, "golden domain key changed");
    });

    crate::contract_test!(const_u32_frame_matches_runtime_child {
        // Load-bearing drift guard, same contract as
        // const_frame_matches_runtime_child: const u32 framing must hash to
        // the key the runtime CXS codec derives, or const-derived field
        // keys diverge from runtime-derived ones.
        macro_rules! check {
            ($v:expr) => {{
                const FRAMED: FramedComponent = FramedComponent::from_u32($v);
                let runtime = Keypath::default().child(&$v);
                assert_eq!(
                    runtime.generate_key(),
                    oracle_key(FRAMED.as_bytes()),
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
    });

    crate::contract_test!(child_framed_matches_child {
        const FRAME: FramedComponent = FramedComponent::from_u32(7u32);
        // A non-empty parent (domain + one level) so the append lands on a
        // realistic preimage.
        let parent = Keypath::default().child("Domain").child(&3u32);
        let via_const = parent.child_framed(&FRAME);
        let via_runtime = parent.child(&7u32);
        assert!(via_const.addresses_same_slot(&via_runtime));
        // It is a child, not the parent.
        assert!(!via_const.addresses_same_slot(&parent));
    });

    crate::contract_test!(child_framed_preserves_exclusive_flag {
        const FRAME: FramedComponent = FramedComponent::from_u32(0u32);
        let mut parent = Keypath::default().child("Domain");
        parent.set_exclusive();
        // child_framed clones the parent, so access mode propagates exactly
        // like child() (which also clones).
        assert!(parent.child_framed(&FRAME).is_exclusive());
    });

    crate::contract_test!(const_frame_matches_runtime_child {
        // THE load-bearing test: const framing must hash to the key the
        // runtime CXS codec derives, or const-derived storage keys diverge
        // from runtime-derived ones.
        macro_rules! check {
            ($domain:literal) => {{
                const N: usize = framed_domain_len($domain);
                const FRAMED: [u8; N] = frame_domain::<N>($domain);
                let runtime = Keypath::default().child($domain);
                assert_eq!(
                    runtime.generate_key(),
                    oracle_key(&FRAMED),
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
    });
}

pub use leaf_key::*;
mod leaf_key {
    use super::Keypath;
    use crate::types::StorageKey;
    use core::ops::Deref;
    use groundhog_xdr::storage_item::StorageDeltaFlags;

    /// The derived storage key a leaf storage cell holds, plus its access
    /// mode. Construction derives the key immediately and releases the
    /// keypath's VM hash stream, so leaves hold no stream — only containers
    /// that keep deriving child slots (maps, vectors) retain a [`Keypath`].
    #[derive(Debug)]
    pub struct LeafKey {
        key: StorageKey,
        exclusive: bool,
    }

    impl From<Keypath> for LeafKey {
        fn from(keypath: Keypath) -> Self {
            let exclusive = keypath.is_exclusive();
            LeafKey {
                key: keypath.into_key(),
                exclusive,
            }
        }
    }

    impl LeafKey {
        pub fn get(&self) -> &StorageKey {
            &self.key
        }

        pub fn is_exclusive(&self) -> bool {
            self.exclusive
        }

        /// The flags each write through this leaf requests after it
        /// succeeds; see [`Keypath::write_flags`].
        pub fn write_flags(&self) -> StorageDeltaFlags {
            if self.exclusive {
                StorageDeltaFlags::ExclusiveLock
            } else {
                StorageDeltaFlags::None
            }
        }
    }

    impl Deref for LeafKey {
        type Target = StorageKey;

        fn deref(&self) -> &Self::Target {
            &self.key
        }
    }
}
