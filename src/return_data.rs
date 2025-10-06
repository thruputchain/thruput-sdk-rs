pub use implementation::*;

// #[cfg(not(feature = "dynamic_bufs"))]

// create
// decode filled
// write all MU and set filled len

mod implementation {
    pub struct ReturnData {
        buf: core::io::BorrowedBuf<'static>,
    }

    // Get a handle to the static return_data buffer.
    //
    // Safety: Don't hold more than one handle returned by this function.
    pub unsafe fn static_return_data() -> ReturnData {
        use core::mem::MaybeUninit as MU;

        const SZ: usize = 200;

        static mut BYTES: [MU<u8>; SZ] = [const { MU::uninit() }; SZ];

        // SAFETY: If the user promises never to hold more than one value
        // returned by this function, then there will not be multiple aliases
        // of the static buffer.
        let bytes: &'static mut [MU<u8>; SZ] = unsafe {
            &mut *(&raw mut BYTES)
        };

        ReturnData { buf: bytes.as_mut_slice().into() }
    }

    impl ReturnData {
        pub fn filled(&self) -> &[u8] {
            self.buf.filled()
        }

        pub fn as_mut(&self) -> &mut [u8] {
        }

        pub fn set_len(
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
