//! System functions: invoke, client_invoke
/*
#[cfg(feature = "dynamic_bufs")]
mod return_buf {
    // A dynamically-sized (via reallocation) buffer

    static mut RETURN_BUF: ReturnBuf = ReturnBuf::new();
    // Vec::with_capacity() is not const, otherwise we'd pre-allocate

    pub fn return_buf_mut() -> &'static mut [u8] {
        let ptr = &raw mut RETURN_BUF;

        let buf = unsafe { &mut *ptr };

        buf.as_mut()
    }

    pub use return_buf_mut as return_buf;
    /*
    pub fn return_buf() -> &'static [u8] {
        let ptr = unsafe { &raw RETURN_BUF };
        unsafe { RETURN_BUF.as_ref() }
    }
    */
}
*/

// #[cfg(not(feature = "dynamic_bufs"))]
// mod return_buf 
/*
// #[cfg(not(feature = "dynamic_bufs"))]
mod return_buf {
    // A statically-allocated buffer with a fixed maximum size

    use core::mem::MaybeUninit as MU;

    const SZ: usize = 500;

    pub struct ReturnBuf {
        contents: [MU<u8>; SZ],
        len: u32,
    }

    impl ReturnBuf {
        pub const fn new() -> Self {
            Self {
                contents: [const { MU::uninit() }; SZ],
                len: 0
            }
        }

        pub fn set_len(&mut self, len: u32) {
            self.len = len;
        }

    }

    impl AsMut<[u8]> for ReturnBuf {
        fn as_mut(&mut self) -> &mut [u8] {
            let ptr = MU::array_assume_init(self.contents).as_mut_ptr();

            unsafe { &mut *ptr }
        }
    }
}
*/
/*
#[cfg(not(feature = "dynamic_bufs"))]
mod return_buf {
    // A statically-allocated buffer with a fixed maximum size
    type ReturnBuf = heapless::Vec<u8, 500, u32>;


    static mut RETURN_BUF: ReturnBuf = ReturnBuf::new();

    pub fn return_buf_mut() -> &'static mut [MU<u8>] {
        let ptr = &raw mut RETURN_BUF;
        let buf = unsafe { &mut *ptr };

        buf.spare_capacity_mut()
    }

    pub use return_buf_mut as return_buf;
    /*
    pub fn return_buf() -> &'static [u8] {
        unsafe { RETURN_BUF.as_ref() }
    }
    */

    pub fn return_buf_set_len(len: usize) {
        let ptr = &raw mut RETURN_BUF;
        let buf = unsafe { &mut *ptr };

        buf.set_len(len)
    }
}
*/

// use return_buf::*;

use crate::sys;
use crate::types::{ ContractAddress, RuntimeMemoryLength };
use crate::contract::ContractEntrypointID;
use crate::contract_panic;
use xdrs::{ReadXDROwned, WriteXDR};
use xdrs::serialize_txs::{encode, decode};
use core::error::Error;

pub use crate::invoke_error::*;

type ReturnData = core::io::BorrowedBuf<'static>;

// Get a handle to the static return_data buffer.
//
// Safety: Don't hold more than one handle returned by this function.
unsafe fn static_return_data() -> ReturnData {
    use core::mem::MaybeUninit as MU;

    const SZ: usize = 200;

    static mut BYTES: [MU<u8>; SZ] = [const { MU::uninit() }; SZ];

    // SAFETY: If the user promises never to hold more than one value
    // returned by this function, then there will not be multiple aliases
    // of the static buffer.
    let bytes: &'static mut [MU<u8>; SZ] = unsafe {
        &mut *(&raw mut BYTES)
    };

    bytes.as_mut_slice().into()
}

pub fn invoke<ARGS, RET>(
    address: &ContractAddress,
    entrypoint: ContractEntrypointID,
    gas_limit: u64,
    args: &ARGS,
) -> InvokeResult<RET>
where
    ARGS: WriteXDR,
    RET: ReadXDROwned,
{
    // Encode the args
    let call_data = encode(args).or(Err(InvokeError::EncodeArgs))?;

    // Invoke the method
    let return_data = invoke_inner(address, entrypoint, gas_limit, call_data.as_slice())?;

    // Deserialize the return data into a return value ...
    let return_val = decode(return_data)
        .map_err(|_| InvokeError::DecodeReturnVal)?;

    Ok(return_val)
}

// At least this part won't be monomorphized multiple times
#[inline(never)]
fn invoke_inner(
    address: &ContractAddress,
    entrypoint: ContractEntrypointID,
    gas_limit: u64,
    call_data: &[u8],
) -> InvokeResult<&'static [u8]> {
    // Perform the cross-contract call

    let mut return_len = 0;
    let entrypoint = u64::from_be_bytes(entrypoint.into_inner());

    let result = sys::raw_Invoke(
        address,
        entrypoint,
        gas_limit,
        call_data,
        0, // TODO(geoff): invoke flags go here
        &mut return_len,
    ).map_err(InvokeError::Syscall);

    let is_ok = match result {
        Ok(()) => true,
        Err(InvokeError::Syscall(syserr)) if syserr.is_aborted() => false,
        Err(e) => {
            return Err(e)
        }
    };

    // Ok or abort: get the return data from the VM ...
 
    // SAFETY: This is the only call-site of static_return_data(), and this
    // function does not re-enter
    let mut return_data = unsafe { static_return_data() };
    let mut cursor = return_data.unfilled();

    // SAFETY: We promise not to uninitialize any bytes
    let cursor_mut = unsafe { cursor.as_mut() };

    let mut written: RuntimeMemoryLength = 0;

    sys::uninit_GetReturnedBuffer(cursor_mut, &mut written)
        .unwrap_or_else(|_| contract_panic!("couldn't get return data"));

    if written != return_len {
        contract_panic!("unexpected return data length; written = {written}, \
            return_len = {return_len}");
    }

    // SAFETY: The VM reports above that it has initialized this many bytes of the cursor
    unsafe {
        cursor.set_init(written as usize);
    }

    // We've filled this much of return_data
    cursor.advance(written as usize);

    if is_ok {
        Ok(return_data.into_filled())
    } else {
        Err(InvokeError::Aborted)
    }
}

// Invoke a client contract where we know the error type (which
// is produced on abort)
pub fn client_invoke<ARGS, RET, ERR>(
    address: &ContractAddress,
    entrypoint: ContractEntrypointID,
    gas_limit: u64,
    args: &ARGS,
) -> ClientResult<RET, ERR>
where
    ARGS: WriteXDR,
    RET: ReadXDROwned,
    ERR: ReadXDROwned + WriteXDR + Error,
{
    match invoke(address, entrypoint, gas_limit, args) {
        Ok(ret) => Ok(ret),
        Err(err) => {
            match err {
                InvokeError::Aborted => {
                    let return_data = unsafe { static_return_data() };
                    match decode(return_data.filled()) {
                        Ok(val) => Err(ClientError::Aborted(val)),
                        Err(_) => Err(ClientError::AbortDecodeFailed),
                    }
                }
                _ => Err(ClientError::Invoke(err))
            }
        }
    }
}
