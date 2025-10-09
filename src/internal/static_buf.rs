// Declares a type alias for a handle to an MUBuf of a particular size, and also a function that
// returns such a handle to a single, statically-allocated MBuf.  Attempting to hold more than one
// such handle will cause a panic.
#[macro_export]
macro_rules! static_mu_buf {
    { $( $vis:vis fn $fn:ident<$size:ident>($size_hint:ident: Option<usize>) -> $ty:ident; )* } => { $(

        $vis type $ty = core::cell::RefMut<'static, crate::internal::MUBuf<$size>>;

        $vis fn $fn(_size_hint: Option<usize>) -> $ty {
            use core::cell::RefCell;
            use $crate::internal::MUBuf;

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
#[cfg(feature = "alloc")]
#[macro_export]
macro_rules! static_vec {
    { $( $vis:vis fn $fn:ident($size_hint:ident: Option<usize>) -> $ty:ident; )* } => { $(

        use alloc::vec::Vec;
        use core::cell::RefCell;

        $vis type $ty = core::cell::RefMut<'static, Vec<u8>>;

        $vis fn $fn(size_hint: Option<usize>) -> $ty {

            static mut VEC: RefCell<Vec<u8>> = RefCell::new(Vec::new());

            // SAFETY: We're single-threaded, so the RefCell is enough to prevent aliasing (at runtime)
            let mut vec = unsafe {
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

/* In some cases we could avoid the mutex and just do:
 *
    const SZ: usize = 1000;
    static mut BUF: MaybeUninit<[u8; SZ]> = MaybeUninit::uninit();

    let mu_ptr: *mut MaybeUninit<[u8; SZ]> = &raw mut BUF;
    let ref: &mut [u8; SZ] = unsafe {
        (*mu_ptr).assume_init_mut()
    };
*/
