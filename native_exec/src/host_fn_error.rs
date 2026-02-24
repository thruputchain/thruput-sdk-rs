use hogexec_internal::ffi_enums::HostFnErrorCode;
use std::panic::panic_any;
use groundhog_xdr::syscalls::SysErr;
use groundhog_xdr::SyscallNo;

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
    println!("performing mocked syscall: {}", syscall);
    let result: groundhog_sys::ForeignHostFnResult = unsafe {
        groundhog_sys::mock_syscall_entrypoint(
            syscall, arg1, arg2, arg3, arg4, arg5, arg6, arg7,
        )
    };

    match result.panic {
        x if x == HostFnErrorCode::kHostNoneOrRecoverable as i32 => {
            result.result_code
        }
        x if x == HostFnErrorCode::kHostOutOfGas as i32 => {
            panic_any(crate::invoke_error::out_of_gas_error());
        }
        x if x == HostFnErrorCode::kHostUnrecoverable as i32 => {
            panic_any(crate::invoke_error::unrecoverable_error());
        }
        x if x == HostFnErrorCode::kHostReturn as i32 => {
            panic_any(crate::invoke_error::return_error());
        }
        x if x == HostFnErrorCode::kHostExit as i32 => {
            panic_any(crate::invoke_error::exit_error());
        }
        x if x == HostFnErrorCode::kHostValidationFailure as i32 => {
            panic_any(crate::invoke_error::validation_error());
        }
        x if x == HostFnErrorCode::kHostAbortTransaction as i32 => {
            panic_any(crate::invoke_error::abort_transaction_error());
        }
        _ => {
            panic_any(crate::invoke_error::unrecoverable_error());
        }
    }
}

#[unsafe(no_mangle)]
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
