//! The runtime's arithmetic host operations, for host tests.
//!
//! `thruput-u256` divides and takes square roots only through the runtime's
//! `U256MulDivRem`, `U128MulDivRem` and `U256Sqrt`. A host test has no
//! runtime, so each test binary links this in its place, as a native test
//! links the native executor's `syscall_wrapper_impl`. `uint` does the
//! arithmetic.

// `uint::construct_uint!` trips this in its own expansion.
#![allow(clippy::manual_div_ceil)]

use thruput_xdr::SyscallNo;
use thruput_xdr::syscall_error::SyscallErrorCode;

uint::construct_uint! {
    /// Wide enough for the 512-bit product the 256-bit call divides.
    struct Wide(8);
}

/// Reads `words` 64-bit words at `ptr`, least significant first.
fn read(ptr: u64, words: usize) -> Wide {
    // SAFETY: the bindings pass a pointer to a live value of that size.
    let bytes =
        unsafe { core::slice::from_raw_parts(ptr as *const u8, words * 8) };
    let mut value = [0u64; 8];
    for (word, chunk) in value.iter_mut().zip(bytes.as_chunks::<8>().0) {
        *word = u64::from_le_bytes(*chunk);
    }
    Wide(value)
}

/// Writes the low `words` 64-bit words of `value` at `ptr`.
fn write(ptr: u64, value: Wide, words: usize) {
    // SAFETY: the bindings pass a pointer to an output of that size.
    let bytes =
        unsafe { core::slice::from_raw_parts_mut(ptr as *mut u8, words * 8) };
    for (chunk, word) in bytes.as_chunks_mut::<8>().0.iter_mut().zip(value.0) {
        *chunk = word.to_le_bytes();
    }
}

#[unsafe(no_mangle)]
pub extern "C-unwind" fn syscall_wrapper_impl(
    syscall_no: u64,
    arg1: u64,
    arg2: u64,
    arg3: u64,
    arg4: u64,
    arg5: u64,
    arg6: u64,
    _arg7: u64,
) -> i32 {
    let words = match syscall_no as u32 {
        n if n == SyscallNo::kU256Sqrt.0 => {
            write(arg2, read(arg1, 4).integer_sqrt(), 2);
            return SyscallErrorCode::kSyscallErrorCodeNone.0;
        }
        n if n == SyscallNo::kU256MulDivRem.0 => 4,
        n if n == SyscallNo::kU128MulDivRem.0 => 2,
        n => panic!("no host test model of syscall {n}"),
    };
    let (a, b, divisor) =
        (read(arg1, words), read(arg2, words), read(arg3, words));
    if divisor.is_zero() {
        return SyscallErrorCode::kSyscallErrorCodeInvalidSyscallInput.0;
    }
    let (quotient, remainder) = (a * b).div_mod(divisor);
    write(arg4, quotient, words);
    write(arg5, quotient >> (64 * words), words);
    write(arg6, remainder, words);
    SyscallErrorCode::kSyscallErrorCodeNone.0
}
