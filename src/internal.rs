pub use mu_buf::*;

mod mu_buf {
    use core::mem::MaybeUninit as MU;

    pub trait MUBuffer {
        /// Get a reference to the filled/initialized portion of the buffer
        ///
        /// _Safety_: Prior use of the `as_mut` and `set_len` methods must
        /// not have left any uninitialized elements in the portion up to
        /// the stated length.
        unsafe fn filled(&self) -> &[u8];

        /// Get a reference to the maybe-uninitialized buffer
        fn as_mut(&mut self) -> &mut [MU<u8>];

        /// Assert that all elements up to `len - 1` have been initialized
        fn set_len(&mut self, len: usize);
    }

    /// A buffer with an unsafe interface for initializing
    pub struct MUBuf<const SIZE: usize> {
        buf: [MU<u8>; SIZE],
        len: usize,
    }

    impl<const SIZE: usize> MUBuf<SIZE> {
        pub const fn new() -> Self {
            Self {
                len: 0,
                buf: [const { MU::uninit() }; SIZE],
            }
        }
    }

    impl<const SIZE: usize> MUBuffer for MUBuf<SIZE> {
        /// Get a reference to the filled/initialized portion of the buffer
        ///
        /// _Safety_: Prior use of the `as_mut` and `set_len` methods must
        /// not have left any uninitialized elements in the portion up to
        /// the stated length.
        unsafe fn filled(&self) -> &[u8] {
            let filled = &self.buf[..self.len];

            unsafe {
                filled.assume_init_ref()
            }
        }

        /// Get a reference to the maybe-uninitialized buffer
        fn as_mut(&mut self) -> &mut [MU<u8>] {
            &mut self.buf[..]
        }

        /// Assert that all elements up to `len - 1` have been initialized
        fn set_len(&mut self, len: usize) {
            self.len = len;
        }
    }
}

// Declares a type alias for a StaticMURefMut of a certain size,
// and also a function for retrieving it.  Attempting to hold
// more than one value returned from the function will result
// in a panic.
#[macro_export]
macro_rules! static_mu_buf {
    { $( $vis:vis $fn:ident<$size:literal>() -> $ty:ident; )* } => { $(

        $vis type $ty = core::cell::RefMut<'static, crate::internal::MUBuf<$size>>;

        $vis fn $fn() -> $ty {
            pub use core::cell::RefCell;

            static mut MU_BUF: RefCell<crate::internal::MUBuf<$size>> =
                RefCell::new(crate::internal::MUBuf::new());

            // SAFETY: We're single-threaded, so the RefCell is enough to prevent aliasing (at runtime)
            unsafe {
                #[allow(static_mut_refs)]
                MU_BUF.try_borrow_mut().unwrap_or_else(|e| {
                    panic!("{}: attempted multiple concurrent borrows of static buffer: {}",
                        stringify!($fn), e)
                })
            }
        }

    )* };
}
