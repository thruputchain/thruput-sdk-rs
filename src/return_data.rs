pub use implementation::*;

// #[cfg(not(feature = "dynamic_bufs"))]

// create
// decode filled
// write all MU and set filled len

mod implementation {
    use core::mem::MaybeUninit as MU;
    use core::cell::*;

    const SZ: usize = 200;

    static mut BUF: [MU<u8>; SZ] = [const { MU::uninit() }; SZ];

    pub struct Data {
        len: usize,
    }

    pub type ReturnData = RefMut<'static, Data>;

    static mut RETURN_DATA: RefCell<Data> = RefCell::new(Data { len: 0 });

    pub fn static_return_data() -> RefMut<'static, Data> {
        #[allow(static_mut_refs)]
        // SAFETY: We're single-threaded, so the RefCell is enough to prevent aliasing (at runtime)
        unsafe { RETURN_DATA.borrow_mut() }
    }

    impl Data {
        // XXX: take mut ref?
        pub unsafe fn filled(&self) -> &[u8] {
            // SAFETY: The user is not allowed to have more than one `ReturnData`
            let buf = unsafe { &mut *(&raw mut BUF) };

            let filled = &buf[..self.len];

            // SAFETY: The user of `ReturnData` has asserted that self.len bytes are initialized
            unsafe {
                filled.assume_init_ref()
            }
        }

        pub unsafe fn as_mut(&mut self) -> &mut [MU<u8>] {
            // SAFETY: The user is not allowed to have more than one `ReturnData`
            let buf = unsafe { &mut *(&raw mut BUF) };

            &mut buf[..]
        }

        pub unsafe fn set_len(&mut self, len: usize) {
            self.len = len;
        }
    }
}

/*
// An arbitrary-size buffer, adjusted dynamically by reallocation.
// The handle (a fat-pointer) is statically allocated
#[cfg(feature = "dynamic_bufs")]
mod return_data {

    #[cfg(not(feature = "alloc"))]
    compile_error!("feature \"dynamic bufs\" requires liballoc, provided by feature \"alloc\"");

    use core::cell::Cell;
    use alloc::vec::Vec;

    pub type ReturnData = &'static Vec<u8>;

    // Get a handle to the static return_data buffer.
    //
    // Safety: Don't hold more than one handle returned by this function.
    pub unsafe fn static_return_data() -> ReturnData {
        static mut BYTES: Vec<u8> = Vec::new();

        // SAFETY: If the user promises never to hold more than one value
        // returned by this function, then there will not be multiple aliases
        // of the static buffer.
        unsafe {
            &mut *(&raw mut BYTES)
        }
    }
}
*/
