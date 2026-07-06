#![allow(clippy::len_without_is_empty)]
#![allow(clippy::new_without_default)]

//! Maybe-uninitialized buffers of static size
use core::mem::MaybeUninit as MU;

/// A buffer with an unsafe interface for initializing.
///
/// This is the SDK's single fixed-capacity byte buffer: under `alloc` the
/// scratch/return buffers are a heap `Vec`, but in the no-alloc guest they
/// are all `MUBuf`. Its surface is a deliberate subset of the `Vec`
/// interface so the same call sites work against either.
pub struct MUBuf<const SIZE: usize> {
    buf: [MU<u8>; SIZE],
    len: usize,
}

impl<const SIZE: usize> MUBuf<SIZE> {
    /// Create an uninitialized buffer
    pub const fn new() -> Self {
        Self {
            len: 0,
            buf: [const { MU::uninit() }; SIZE],
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    /// The fixed capacity. Mirrors `Vec::capacity` so receive paths can bound
    /// the host-reported length the same way under both buffer types.
    pub const fn capacity(&self) -> usize {
        SIZE
    }

    /// Reset the logical length to zero. The backing bytes are left as-is
    /// (they are `MaybeUninit` and gated by `len`).
    pub fn clear(&mut self) {
        self.len = 0;
    }

    /// Get a mutable reference to the maybe-uninitialized buffer
    pub fn spare_capacity_mut(&mut self) -> &mut [MU<u8>] {
        &mut self.buf[..]
    }

    /// Append `other` after the initialized region. Returns `Err(())` if it
    /// would overflow the fixed capacity (signature mirrors `heapless::Vec`).
    #[allow(clippy::result_unit_err)]
    pub fn extend_from_slice(&mut self, other: &[u8]) -> Result<(), ()> {
        let end = self.len + other.len();
        if end > SIZE {
            return Err(());
        }
        for (dst, &src) in self.buf[self.len..end].iter_mut().zip(other) {
            dst.write(src);
        }
        self.len = end;
        Ok(())
    }

    /// The initialized portion as a byte slice. Alias for `AsRef` so call
    /// sites can use `as_slice()` uniformly with `Vec`.
    pub fn as_slice(&self) -> &[u8] {
        self.as_ref()
    }

    /// Assert that all elements up to `len - 1` have been initialized
    ///
    /// # Safety
    ///
    /// The given length must correctly state the length of initialized data before any use of `as_ref`.
    pub unsafe fn set_len(&mut self, len: usize) {
        self.len = len;
    }
}

impl<const SIZE: usize> Default for MUBuf<SIZE> {
    fn default() -> Self {
        Self::new()
    }
}

// Compare/format by the initialized region only, so `MUBuf` is a drop-in for
// the heap `Vec` that backs `ScratchBuf` under `alloc` (needed by the
// `#[derive(Debug, PartialEq)]` on `ContractError` in
// `crate::call::contract_result`, whose `Abort` variant holds an `AbortBuf`
// — a `ScratchBuf`, i.e. this `MUBuf` in the no-alloc guest).
impl<const SIZE: usize> PartialEq for MUBuf<SIZE> {
    fn eq(&self, other: &Self) -> bool {
        self.as_ref() == other.as_ref()
    }
}

impl<const SIZE: usize> Eq for MUBuf<SIZE> {}

impl<const SIZE: usize> core::fmt::Debug for MUBuf<SIZE> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(self.as_ref(), f)
    }
}

impl<const SIZE: usize> AsRef<[u8]> for MUBuf<SIZE> {
    // Get a reference to the filled/initialized portion of the buffer
    fn as_ref(&self) -> &[u8] {
        let filled_mu = &self.buf[..self.len];

        unsafe { filled_mu.assume_init_ref() }
    }
}
