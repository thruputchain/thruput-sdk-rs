use crate::bindings::mock_syscall_entrypoint;
use crate::bindings::ForeignHostFnResult;
use groundhog_xdr::ffi_enums::HostFnErrorCode;
use std::panic::panic_any;

impl ForeignHostFnResult {
    pub fn unwrap(&self) -> i32 {
        match self.panic {
            x if x == HostFnErrorCode::kHostNoneOrRecoverable as i32 => {
                return self.result_code;
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
            _ => {
                panic_any(crate::invoke_error::unrecoverable_error());
            }
        }
    }
}

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
    let result: ForeignHostFnResult = unsafe {
        mock_syscall_entrypoint(
            syscall, arg1, arg2, arg3, arg4, arg5, arg6, arg7,
        )
    };

    result.unwrap()
}
