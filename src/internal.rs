pub use mu_buf::*;

mod mu_buf {
    use core::mem::MaybeUninit as MU;

    /*
    pub trait MUBuffer {
        /// Get a reference to the filled/initialized portion of the buffer
        fn as_ref(&self) -> &[u8];

        /// Get a reference to the maybe-uninitialized buffer
        fn spare_capacity_mut(&mut self) -> &mut [MU<u8>];

        /// Assert that all elements up to `len - 1` have been initialized
        ///
        /// _Safety_: This length must correctly state the length of initialized
        /// data before any use of `as_ref`
        unsafe fn set_len(&mut self, len: usize);
    }
    */

    /// A buffer with an unsafe interface for initializing
    pub struct MUBuf<const SIZE: usize> {
        buf: [MU<u8>; SIZE],
        len: usize,
    }

    // Note that this is a subset of the Vec interface
    impl<const SIZE: usize> MUBuf<SIZE> {
        pub const fn new() -> Self {
            Self {
                len: 0,
                buf: [const { MU::uninit() }; SIZE],
            }
        }

        pub fn as_ref(&self) -> &[u8] {
            let filled = &self.buf[..self.len];

            unsafe {
                filled.assume_init_ref()
            }
        }

        pub fn spare_capacity_mut(&mut self) -> &mut [MU<u8>] {
            &mut self.buf[..]
        }

        pub unsafe fn set_len(&mut self, len: usize) {
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
    { $( $vis:vis $fn:ident<$size:ident>($size_hint:ident: usize) -> $ty:ident; )* } => { $(

        $vis type $ty = core::cell::RefMut<'static, crate::internal::MUBuf<$size>>;

        $vis fn $fn(_size_hint: usize) -> $ty {
            use core::cell::RefCell;
            use crate::internal::MUBuf;

            static mut BUF: RefCell<MUBuf<$size>> =
                RefCell::new(MUBuf::new());

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
