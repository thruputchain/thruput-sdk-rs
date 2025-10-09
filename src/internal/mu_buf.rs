//! Maybe-uninitialized buffers of static size
use core::mem::MaybeUninit as MU;

/// A buffer with an unsafe interface for initializing
pub struct MUBuf<const SIZE: usize> {
    buf: [MU<u8>; SIZE],
    len: usize,
}

// (Note that this is a subset of the Vec interface)
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

    /// Get a mutable reference to the maybe-uninitialized buffer
    /// TODO: change to `capacity_mut()`, and make a trait for this interface?
    pub fn spare_capacity_mut(&mut self) -> &mut [MU<u8>] {
        &mut self.buf[..]
    }

    /// Assert that all elements up to `len - 1` have been initialized
    ///
    /// _Safety_: This length must correctly state the length of initialized
    /// data before any use of `as_ref`
    pub unsafe fn set_len(&mut self, len: usize) {
        self.len = len;
    }
}

impl<const SIZE: usize> AsRef<[u8]> for MUBuf<SIZE> {
    // Get a reference to the filled/initialized portion of the buffer
    fn as_ref(&self) -> &[u8] {
        let filled = &self.buf[..self.len];

        unsafe {
            filled.assume_init_ref()
        }
    }
}
