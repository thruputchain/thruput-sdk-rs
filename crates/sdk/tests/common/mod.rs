#![allow(dead_code)]

use thruput_sdk::call::RawCallError;
use thruput_sdk::exn::{Exn, LibraryExn};
use thruput_sdk::sys::{SyscallErrorCode, SyscallNo};

/// Decode a LibraryExn from an exception payload (which may have trailing
/// debug info when the `debugging` feature is active).
pub fn decode_library_exn(payload: &[u8]) -> LibraryExn {
    use thruput_sdk::ReadXDR;
    let mut reader = xdrs::cxs::buf_reader::BufReader::new(payload);
    ReadXDR::read_xdr(&mut reader)
        .expect("Failed to decode LibraryExn from exception payload")
}

/// Assert that a RawCallError is an Exception whose payload decodes to a
/// LibraryExn::Syscall matching the given syscall number and error code.
pub fn assert_syscall_exception(
    res: &Result<(), RawCallError>,
    callno: SyscallNo,
    errno: SyscallErrorCode,
) {
    match res {
        Err(RawCallError::Exception(exn))
            if exn.selector() == LibraryExn::SELECTOR =>
        {
            let lib = decode_library_exn(exn.payload());
            match &lib {
                LibraryExn::Syscall(se) if se.is(callno, errno) => {
                    println!("OK: {:?}", lib)
                }
                _ => panic!(
                    "Expected Syscall({:?}, {:?}), got: {:?}",
                    callno, errno, lib
                ),
            }
        }
        _ => panic!("Unexpected result: {:?}", res),
    }
}
