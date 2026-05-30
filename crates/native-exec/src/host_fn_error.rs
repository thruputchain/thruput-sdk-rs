use groundhog_xdr::syserr::SysErr;
use hogexec_internal::ffi_enums::HostFnErrorCode;
use std::panic::panic_any;

#[allow(clippy::too_many_arguments)]
pub fn perform_mocked_syscall(
    syscall: i32,
    arg1: u64,
    arg2: u64,
    arg3: u64,
    arg4: u64,
    arg5: u64,
    arg6: u64,
    arg7: u64,
) -> i32 {
    let result: groundhog_sys::ForeignHostFnResult = unsafe {
        groundhog_sys::mock_syscall_entrypoint(
            syscall, arg1, arg2, arg3, arg4, arg5, arg6, arg7,
        )
    };

    match result.panic {
        x if x == HostFnErrorCode::NoneOrRecoverable.0 => result.result_code,
        x if x == HostFnErrorCode::OutOfGas.0 => {
            panic_any(crate::invoke_error::out_of_gas_error());
        }
        x if x == HostFnErrorCode::Unrecoverable.0 => {
            panic_any(crate::invoke_error::unrecoverable_error());
        }
        x if x == HostFnErrorCode::Return.0 => {
            panic_any(crate::invoke_error::return_error());
        }
        x if x == HostFnErrorCode::Exit.0 => {
            panic_any(crate::invoke_error::exit_error());
        }
        x if x == HostFnErrorCode::ValidationFailure.0 => {
            panic_any(crate::invoke_error::validation_error());
        }
        x if x == HostFnErrorCode::Abandon.0 => {
            panic_any(crate::invoke_error::abort_transaction_error());
        }
        _ => {
            panic_any(crate::invoke_error::unrecoverable_error());
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C-unwind" fn syscall_wrapper_impl(
    // Matches groundhog-xdr's `syscalls_mock` extern declaration, which passes
    // `SyscallNo` widened to a native `SyscallWord` (u64). `SyscallNo` itself is
    // now `repr(transparent)` over a `u32`, so taking it directly here would be
    // an ABI mismatch.
    syscall_no: u64,
    arg1: u64,
    arg2: u64,
    arg3: u64,
    arg4: u64,
    arg5: u64,
    arg6: u64,
    arg7: u64,
) -> SysErr {
    SysErr(perform_mocked_syscall(
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
