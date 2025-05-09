use core::hint::unreachable_unchecked;
use core::mem::MaybeUninit;

use crate::syscall::{SysErr, SyscallResult};
use groundhog_xdr::types::*;
use groundhog_xdr::transaction::*;

macros::generate_syscall_numbers!("protocol-rs/protocol/syscalls.yaml");
macros::generate_syscall_error_codes!("protocol-rs/protocol/syscalls.yaml");

#[cfg(not(feature = "mock"))]
#[link(wasm_import_module = "marmot")]
extern "C" {
    pub(crate) fn syscall(
        syscall: u64,
        arg1: u64,
        arg2: u64,
        arg3: u64,
        arg4: u64,
        arg5: u64,
        arg6: u64,
        arg7: u64,
    ) -> i32;
}

#[cfg(not(feature = "mock"))]
fn syscall_wrapper(
    syscall_no: SyscallNo,
    arg1: u64,
    arg2: u64,
    arg3: u64,
    arg4: u64,
    arg5: u64,
    arg6: u64,
    arg7: u64,
) -> SysErr {
    unsafe {
        let errno =
            syscall(syscall_no as u64, arg1, arg2, arg3, arg4, arg5, arg6, arg7);
        SysErr(errno)
    }
}

#[cfg(feature = "mock")]
fn syscall_wrapper(
    syscall_no: SyscallNo,
    arg1: u64,
    arg2: u64,
    arg3: u64,
    arg4: u64,
    arg5: u64,
    arg6: u64,
    arg7: u64,
) -> SysErr {
    SysErr(native_exec::perform_mocked_syscall(
        syscall_no as i32,
        arg1,
        arg2,
        arg3,
        arg4,
        arg5,
        arg6,
        arg7,
    ))
}

macros::generate_syscall_decls!("protocol-rs/protocol/syscalls.yaml");
