use crate::types::ForeignHostFnResult;
use groundhog_xdr::ffi_enums::HostFnErrorCode;
use std::panic::panic_any;

use crate::types::mock_syscall_entrypoint;

impl ForeignHostFnResult {
    pub fn unwrap(&self) -> i32 {
        match self.panic {
            x if x == HostFnErrorCode::kNoneOrRecoverable as i32 => {
                return self.result_code;
            }
            x if x == HostFnErrorCode::kOutOfGas as i32 => {
                panic_any(crate::invoke_error::out_of_gas_error());
            }
            x if x == HostFnErrorCode::kUnrecoverable as i32 => {
                panic_any(crate::invoke_error::unrecoverable_error());
            }
            x if x == HostFnErrorCode::kReturn as i32 => {
                panic_any(crate::invoke_error::return_error());
            }
            x if x == HostFnErrorCode::kExit as i32 => {
                panic_any(crate::invoke_error::exit_error(self.result_code));
            }
            x if x == HostFnErrorCode::kValidationFailure as i32 => {
                panic_any(crate::invoke_error::validation_error());
            }
            _ => {
                panic_any(crate::invoke_error::unrecoverable_error());
            }
        }
    }
}

//struct ForeignHostFnResult mock_syscall_entrypoint(int32_t syscall, const unsigned char* input, uint32_t input_len,
//    unsigned char* output, uint32_t output_len);

pub fn perform_mocked_syscall(
    syscall: groundhog_xdr::syscall::SyscallNo,
    input: &[u8],
    output: &mut [u8],
) -> i32 {
    let result: ForeignHostFnResult = unsafe {
        mock_syscall_entrypoint(
            syscall as i32,
            input.as_ptr(),
            input.len().try_into().unwrap(),
            output.as_mut_ptr(),
            output.len().try_into().unwrap(),
        )
    };

    result.unwrap()
}
