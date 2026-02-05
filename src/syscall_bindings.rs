use groundhog_xdr::syscalls::SysErr;
use groundhog_xdr::syscalls::SyscallNo;

#[unsafe(no_mangle)]
#[cfg(feature = "mock")]
pub extern "C-unwind" fn syscall_wrapper_impl(
    syscall_no: SyscallNo,
    arg1: u64,
    arg2: u64,
    arg3: u64,
    arg4: u64,
    arg5: u64,
    arg6: u64,
    arg7: u64,
) -> SysErr {
    SysErr(groundhog_native_exec::perform_mocked_syscall(
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
