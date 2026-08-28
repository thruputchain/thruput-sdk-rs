use crate::crypto::sha256;
use crate::types::StorageKey;
use core::fmt::Debug;
use groundhog_xdr::storage_item::StorageDeltaFlags;
use xdrs::{WriteXDR, cxs::ConstWriter};

/// Keypath preimage budget (total framed bytes across all levels).
pub const STATIC_KEYPATH_CAPACITY: usize = 256;

/// Slot-pool-backed byte buffer for [`Keypath`] preimages — the one
/// implementation on every target.
///
/// A [`Keypath`] whose buffer is a fixed inline array is ~260 bytes, and the
/// storage layer's builder-style API moves keypaths by value many times per
/// storage access. On rv32 every move is a real memcpy loop metered per
/// instruction, which made the typed-storage path ~5,000 instructions per
/// entrypoint before the first syscall (metrics_history#1080). Keeping the
/// bytes in a contract-local pool and handing `Keypath` a one-byte slot
/// handle makes moves free while preserving the no-alloc property: the pool
/// is a static, not a heap.
///
/// Native hosts use one pool per thread, mirroring the guest's
/// per-invocation pool, so native tests enforce exactly the guest's
/// preimage budget and live-slot cap instead of an unbounded heap buffer
/// that would hide on-chain overflows. A terminal Return skips `Drop` and
/// leaks any live slots, which is fine: the frame's memory dies with it.
mod pooled {
    use super::STATIC_KEYPATH_CAPACITY;
    use core::mem::MaybeUninit;

    /// Concurrently-live keypath buffers per invocation frame. Live handles
    /// are roughly the contract state root's total field count (the root
    /// constructs every field's path eagerly) plus a few temporaries per
    /// `at()` chain.
    pub const KEYPATH_POOL_SLOTS: usize = 32;
    const CAP: usize = STATIC_KEYPATH_CAPACITY;

    struct Pool {
        /// Bitmask of live slots (bit set = allocated). Zero-initialized so
        /// the whole pool lands in `.bss` and adds nothing to the stored
        /// initial-memory image. `u32` so guest bookkeeping is single-word.
        used: u32,
        lens: [u16; KEYPATH_POOL_SLOTS],
        bufs: MaybeUninit<[[u8; CAP]; KEYPATH_POOL_SLOTS]>,
    }

    const _: () = assert!(KEYPATH_POOL_SLOTS <= 32);

    const POOL_INIT: Pool = Pool {
        used: 0,
        lens: [0; KEYPATH_POOL_SLOTS],
        bufs: MaybeUninit::uninit(),
    };

    // The guest arm holds the pool in a single static (single-threaded, no
    // interrupts; zero-initialized so it lands in `.bss`). Native hosts run
    // many test threads, so each thread gets its own pool — the per-thread
    // pool plays the role the per-invocation static plays on the guest.
    #[cfg(any(doc, target_arch = "riscv32"))]
    mod pool_cell {
        use super::{POOL_INIT, Pool};
        use core::cell::UnsafeCell;

        struct PoolCell(UnsafeCell<Pool>);
        // SAFETY: single-threaded guest, no interrupts; native hosts use
        // the thread-local arm instead.
        unsafe impl Sync for PoolCell {}

        static POOL: PoolCell = PoolCell(UnsafeCell::new(POOL_INIT));

        pub(super) fn pool() -> *mut Pool {
            POOL.0.get()
        }
    }

    #[cfg(all(not(any(doc, target_arch = "riscv32")), feature = "std"))]
    mod pool_cell {
        use super::{POOL_INIT, Pool};
        use core::cell::UnsafeCell;

        std::thread_local! {
            static POOL: UnsafeCell<Pool> =
                const { UnsafeCell::new(POOL_INIT) };
        }

        pub(super) fn pool() -> *mut Pool {
            // The raw pointer stays valid for the thread's lifetime;
            // thread-local statics never move.
            POOL.with(|p| p.get())
        }
    }

    // Native no-std builds have no thread locals; unreachable in practice
    // (building the sdk natively requires the `std` feature).
    #[cfg(all(not(any(doc, target_arch = "riscv32")), not(feature = "std")))]
    mod pool_cell {
        use super::Pool;

        pub(super) fn pool() -> *mut Pool {
            unreachable!("bug: keypath pool on a no-std native target")
        }
    }

    /// Bits admissible in `used`: the low `KEYPATH_POOL_SLOTS` bits.
    const SLOT_MASK: u32 = if KEYPATH_POOL_SLOTS == 32 {
        u32::MAX
    } else {
        (1u32 << KEYPATH_POOL_SLOTS) - 1
    };

    /// One pooled buffer. The handle is the identity; the bytes live in
    /// [`POOL`]. Not `Copy`: dropping returns the slot.
    pub struct PooledKeyBuf {
        slot: u8,
    }

    fn buf_ptr(slot: u8) -> *mut u8 {
        // Raw pointer projection only — never materialize a reference to the
        // pool, so slices handed out by `as_slice` stay valid across writes
        // to other slots.
        unsafe {
            (&raw mut (*pool_cell::pool()).bufs)
                .cast::<u8>()
                .add(slot as usize * CAP)
        }
    }

    fn len(slot: u8) -> usize {
        unsafe {
            *(&raw const (*pool_cell::pool()).lens)
                .cast::<u16>()
                .add(slot as usize) as usize
        }
    }

    fn set_len(slot: u8, len: usize) {
        unsafe {
            *(&raw mut (*pool_cell::pool()).lens)
                .cast::<u16>()
                .add(slot as usize) = len as u16;
        }
    }

    #[inline]
    fn alloc_slot() -> u8 {
        unsafe {
            let used = &raw mut (*pool_cell::pool()).used;
            let mask = *used;
            if mask == SLOT_MASK {
                // Every slot live: the contract holds more concurrent
                // keypath handles than the pool admits.
                crate::exn::library_throw(
                    crate::exn::LibraryExn::StorageKeypathOverflow,
                );
            }
            let slot = (!mask).trailing_zeros() as u8;
            *used = mask | (1u32 << slot);
            slot
        }
    }

    impl Default for PooledKeyBuf {
        fn default() -> Self {
            let slot = alloc_slot();
            set_len(slot, 0);
            Self { slot }
        }
    }

    /// Out of line for the same reason as [`alloc_slot`]: `child()` clones
    /// at every derivation level.
    #[inline(never)]
    fn clone_slot(from: u8) -> u8 {
        let slot = alloc_slot();
        let n = len(from);
        unsafe {
            core::ptr::copy_nonoverlapping(buf_ptr(from), buf_ptr(slot), n);
        }
        set_len(slot, n);
        slot
    }

    #[inline]
    fn free_slot(slot: u8) {
        unsafe {
            let used = &raw mut (*pool_cell::pool()).used;
            *used &= !(1u32 << slot);
        }
    }

    /// Kept inlinable: append sites have compile-time-known lengths, and
    /// inlining lets the copy specialize instead of looping.
    #[inline]
    fn append_slot(slot: u8, bytes: &[u8]) {
        let start = len(slot);
        let end = start.saturating_add(bytes.len());
        if end > CAP {
            crate::exn::library_throw(
                crate::exn::LibraryExn::StorageKeypathOverflow,
            );
        }
        unsafe {
            core::ptr::copy_nonoverlapping(
                bytes.as_ptr(),
                buf_ptr(slot).add(start),
                bytes.len(),
            );
        }
        set_len(slot, end);
    }

    impl Clone for PooledKeyBuf {
        fn clone(&self) -> Self {
            Self {
                slot: clone_slot(self.slot),
            }
        }
    }

    impl Drop for PooledKeyBuf {
        fn drop(&mut self) {
            free_slot(self.slot);
        }
    }

    impl core::fmt::Debug for PooledKeyBuf {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.debug_struct("PooledKeyBuf")
                .field("slot", &self.slot)
                .field("bytes", &self.as_slice())
                .finish()
        }
    }

    impl PooledKeyBuf {
        /// Append `bytes`, throwing `LibraryExn::StorageKeypathOverflow`
        /// if the preimage budget cannot hold them.
        pub(super) fn append_bytes(&mut self, bytes: &[u8]) {
            append_slot(self.slot, bytes);
        }

        pub(super) fn as_slice(&self) -> &[u8] {
            // SAFETY: bytes `..len` were written by `append_bytes`; the pool
            // is never moved, the slot is uniquely owned by `self`, and
            // writes go through raw pointers so no reference aliases this
            // slice.
            unsafe {
                core::slice::from_raw_parts(buf_ptr(self.slot), len(self.slot))
            }
        }
    }
}

/// Derives storage keys through the VM's incremental-hash streams instead of
/// the one-shot `sha256` syscall, which the host prices per input byte.
///
/// The preimages of sibling slots share their parent's framed prefix, so the
/// module keeps one cached midstate of the most recent parent prefix (absorbed
/// once via `SHA256Update`) and derives each sibling by `SHA256Copy`-forking
/// that midstate and absorbing only the sibling's own final frame. The copy is
/// constant-time in the VM, which is exactly what makes the fork cheaper than
/// re-absorbing the prefix.
///
/// Streams are a scarce per-invocation VM resource shared with user
/// [`Hasher`](crate::hasher::Hasher)s, so this module holds at most one
/// long-lived stream (the cached parent) plus one transient per derivation,
/// both through the ordinary hasher slot allocator. When no slot is available
/// the caller falls back to the one-shot `sha256` syscall — the derived key is
/// identical either way.
mod streamkey {
    use super::STATIC_KEYPATH_CAPACITY;
    use crate::SHA256Hash;
    use crate::hasher::Hasher;

    struct PrefixCache {
        /// Live midstate holding `bytes[..len]` absorbed since its reset.
        /// `None` until the first parented derivation claims a slot; once
        /// claimed, the slot is held for good and the midstate is replaced
        /// in place on a miss. `ManuallyDrop` keeps the native thread-local
        /// cache's teardown from running `Hasher::drop`, whose stream
        /// discard is a syscall no longer backed by a live frame there.
        hasher: Option<core::mem::ManuallyDrop<Hasher>>,
        len: u16,
        bytes: [u8; STATIC_KEYPATH_CAPACITY],
    }

    impl PrefixCache {
        const fn new() -> Self {
            Self {
                hasher: None,
                len: 0,
                bytes: [0; STATIC_KEYPATH_CAPACITY],
            }
        }
    }

    #[cfg(any(doc, target_arch = "riscv32"))]
    mod cache_cell {
        use super::PrefixCache;
        use core::cell::UnsafeCell;

        struct CacheCell(UnsafeCell<PrefixCache>);
        // SAFETY: single-threaded guest, no interrupts (the same argument as
        // `ContractCell`); native hosts use the thread-local arm instead.
        unsafe impl Sync for CacheCell {}

        static CACHE: CacheCell =
            CacheCell(UnsafeCell::new(PrefixCache::new()));

        pub(super) fn with_cache<R>(
            f: impl FnOnce(&mut PrefixCache) -> R,
        ) -> R {
            // SAFETY: single-threaded and non-reentrant — `f` never calls
            // back into this module.
            f(unsafe { &mut *CACHE.0.get() })
        }
    }

    #[cfg(all(not(any(doc, target_arch = "riscv32")), feature = "std"))]
    mod cache_cell {
        use super::PrefixCache;
        use core::cell::RefCell;

        std::thread_local! {
            static CACHE: RefCell<PrefixCache> =
                const { RefCell::new(PrefixCache::new()) };
        }

        pub(super) fn with_cache<R>(
            f: impl FnOnce(&mut PrefixCache) -> R,
        ) -> R {
            CACHE.with(|c| f(&mut c.borrow_mut()))
        }
    }

    // Native no-std builds have no syscall dispatch and no thread locals;
    // `derive` below reports no stream available and the caller falls back.
    #[cfg(all(not(any(doc, target_arch = "riscv32")), not(feature = "std")))]
    mod cache_cell {
        use super::PrefixCache;

        pub(super) fn with_cache<R>(
            _f: impl FnOnce(&mut PrefixCache) -> R,
        ) -> R {
            unreachable!("bug: streamed derivation on a no-syscall target")
        }
    }

    // Every keypath preimage fits in one stream: the pooled buffer caps
    // preimages at STATIC_KEYPATH_CAPACITY, well under the VM's
    // kSHA256BufferMaxSize (4096, hog_exec_constants.x).
    const _: () = assert!(STATIC_KEYPATH_CAPACITY <= 4096);

    /// Hash `pre` through incremental streams, treating `pre[..split]` as the
    /// parent prefix eligible for midstate caching. Returns `None` when no
    /// stream slot is available; the caller must then hash `pre` another way.
    pub(super) fn derive(pre: &[u8], split: usize) -> Option<SHA256Hash> {
        let (parent, tail) = pre.split_at(split);

        let mut t = Hasher::try_new()?;

        // An empty parent has nothing shareable to cache.
        if parent.is_empty() {
            t.update(&pre);
            return Some(t.finish());
        }

        cache_cell::with_cache(|cache| {
            let hit = cache.hasher.is_some()
                && cache.len as usize == parent.len()
                && &cache.bytes[..parent.len()] == parent;
            if !hit {
                // Rebuild the cached midstate for this parent, reusing the
                // held hasher's slot in place when there is one: a reset
                // absorb replaces the underlying stream without touching
                // the slot allocator.
                match cache.hasher.as_mut() {
                    Some(h) => h.reabsorb(parent),
                    None => {
                        let mut h = Hasher::try_new()?;
                        h.update(&parent);
                        cache.hasher = Some(core::mem::ManuallyDrop::new(h));
                    }
                }
                cache.len = parent.len() as u16;
                cache.bytes[..parent.len()].copy_from_slice(parent);
            }
            let parent_hasher: &mut Hasher = cache
                .hasher
                .as_mut()
                .expect("bug: cache entry just ensured");
            if !t.copy_from(parent_hasher) {
                // The cached handle's VM stream is gone: a new invocation
                // reset the VM's streams but not this contract-local cache.
                // Rebuild the midstate in place and retry once; if the copy
                // still fails, derive this key by absorbing the whole
                // preimage into the transient instead.
                parent_hasher.reabsorb(parent);
                if !t.copy_from(parent_hasher) {
                    t.update(&pre);
                    return Some(t.finish());
                }
            }
            t.update(&tail);
            Some(t.finish())
        })
    }
}

/// Path identifying a storage slot.
///
/// A keypath is a sequence of length-framed components appended to a byte
/// buffer. Each call to [`child`](Self::child) appends
/// `CXS(byte_len) ‖ CXS(value)`: the component value CXS-encoded, prefixed by
/// the CXS-encoded byte length of that encoding. The storage key is the
/// SHA-256 of the buffer — computed through the VM's incremental-hash streams
/// when one is available (see the `streamkey` module), through the one-shot
/// `sha256` syscall otherwise, and memoized by [`KeyCache`].
///
/// The self-delimiting `CXS(len)` prefix makes the preimage
/// `child(v1) ‖ child(v2) ‖ …` uniquely decodable: read the length, then read
/// exactly that many value bytes. This holds for *any* value encoding —
/// including fixed opaque `[u8; N]` keys that do not self-delimit — so
/// distinct component sequences can never share a preimage.
///
/// The preimage shares a fixed [`STATIC_KEYPATH_CAPACITY`]-byte budget;
/// exceeding it throws `LibraryExn::StorageKeypathOverflow` (pre-hash
/// oversized keys). A single component's encoding is additionally capped
/// at the SDK's fixed encode-buffer size.
#[derive(Clone, Debug, Default)]
pub struct Keypath {
    buf: pooled::PooledKeyBuf,
    /// Preimage length before the most recent frame was appended: the split
    /// between the parent prefix (shared with sibling slots, eligible for
    /// midstate caching in [`streamkey`]) and this keypath's own final frame.
    /// Zero when the keypath has no parent prefix (root, default).
    last_frame_start: u32,
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
        child.last_frame_start = self.buf.as_slice().len() as u32;
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
        child.last_frame_start = self.buf.as_slice().len() as u32;
        child.buf.append_bytes(frame.as_bytes());
        child
    }

    pub fn generate_key(&self) -> StorageKey {
        let pre = self.buf.as_slice();
        match streamkey::derive(pre, self.last_frame_start as usize) {
            Some(hash) => hash.into(),
            // No stream slot free (the contract holds all hashers): the
            // one-shot syscall derives the identical key.
            None => sha256(pre).into(),
        }
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

    pub fn flags(&self) -> StorageDeltaFlags {
        if self.exclusive {
            StorageDeltaFlags::ExclusiveLock
        } else {
            StorageDeltaFlags::None
        }
    }
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
    fn pooled_buf_fills_to_exact_capacity() {
        let mut s = pooled::PooledKeyBuf::default();
        s.append_bytes(&[7u8; STATIC_KEYPATH_CAPACITY - 2]);
        s.append_bytes(&[1, 2]); // exactly the budget — must succeed
        assert_eq!(s.as_slice().len(), STATIC_KEYPATH_CAPACITY);
    }

    #[test]
    #[should_panic]
    fn pooled_buf_overflow_throws() {
        let mut s = pooled::PooledKeyBuf::default();
        s.append_bytes(&[7u8; STATIC_KEYPATH_CAPACITY]);
        s.append_bytes(&[1]);
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

    #[test]
    fn clone_copies_only_initialized_prefix_and_diverges() {
        let mut s = pooled::PooledKeyBuf::default();
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
        let mut s = pooled::PooledKeyBuf::default();
        let full = [7u8; STATIC_KEYPATH_CAPACITY];
        s.append_bytes(&full);
        let clone = s.clone();
        assert_eq!(clone.as_slice(), s.as_slice());
        assert_eq!(clone.as_slice(), &full[..]);
    }

    #[test]
    fn default_as_slice_is_empty() {
        let s = pooled::PooledKeyBuf::default();
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
