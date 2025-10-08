pub use mu_buf::*;

/// Maybe-uninitialized buffers of static size
mod mu_buf {
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
}

// Declares a type alias for a handle to an MUBuf of a particular size, and also a function that
// returns such a handle to a single, statically-allocated MBuf.  Attempting to hold more than one
// such handle will cause a panic.
#[macro_export]
macro_rules! static_mu_buf {
    { $( $vis:vis fn $fn:ident<$size:ident>($size_hint:ident: Option<usize>) -> $ty:ident; )* } => { $(

        $vis type $ty = core::cell::RefMut<'static, crate::internal::MUBuf<$size>>;

        $vis fn $fn(_size_hint: Option<usize>) -> $ty {
            use core::cell::RefCell;
            use crate::internal::MUBuf;

            static mut BUF: RefCell<MUBuf<$size>> = RefCell::new(MUBuf::new());

            // SAFETY: We're single-threaded, so the RefCell is enough to prevent aliasing (at runtime)
            unsafe {
                #[allow(static_mut_refs)]
                BUF.try_borrow_mut().unwrap_or_else(|e| {
                    panic!("{}: attempted multiple concurrent borrows of static buffer: {}",
                        stringify!($fn), e)
                })
            }
        }

    )* };
}

// Declares a type alias for a handle to a Vec, and also a function that returns such a handle to a
// single, statically-allocated Vec with capacity at least equal to the size hint, if given.
// Attempting to hold more than one such handle will cause a panic.
#[macro_export]
macro_rules! static_vec {
    { $( $vis:vis fn $fn:ident($size_hint:ident: Option<usize>) -> $ty:ident; )* } => { $(

        $vis type $ty = core::cell::RefMut<'static, Vec<u8>>;

        $vis fn $fn(size_hint: Option<usize>) -> Vec<u8> {

            static mut VEC: RefCell<Vec<u8>> = RefCell::new(Vec::new());

            // SAFETY: We're single-threaded, so the RefCell is enough to prevent aliasing (at runtime)
            let vec = unsafe {
                #[allow(static_mut_refs)]
                VEC.try_borrow_mut().unwrap_or_else(|e| {
                    panic!("{}: attempted multiple concurrent borrows of static vec: {}",
                        stringify!($fn), e)
                })
            };

            vec.resize(0, 0); // This sets the zero-base for reserve_exact()
            if let Some(size_hint) = size_hint {
                vec.reserve_exact(size_hint);
            }

            vec
        }

    )* };
}
