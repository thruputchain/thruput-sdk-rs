// -*-Rust-*-

// *** Generated Code  ***
// *** Do not check in ***

use core::convert::TryInto;

use super::{SyscallErrorCode, SysErr};

impl TryInto<SyscallErrorCode> for SysErr {
    type Error = i32;

    fn try_into(self) -> Result<SyscallErrorCode, Self::Error> {
        match self.0 {
            e if e == SyscallErrorCode::kNone as i32 =>
                Ok(SyscallErrorCode::kNone),
            e if e == SyscallErrorCode::kFault as i32 =>
                Ok(SyscallErrorCode::kFault),
            e if e == SyscallErrorCode::kNoSyscall as i32 =>
                Ok(SyscallErrorCode::kNoSyscall),
            e if e == SyscallErrorCode::kIsRootTransaction as i32 =>
                Ok(SyscallErrorCode::kIsRootTransaction),
            e if e == SyscallErrorCode::kContractLookupFailed as i32 =>
                Ok(SyscallErrorCode::kContractLookupFailed),
            e if e == SyscallErrorCode::kContractAlreadyDeployed as i32 =>
                Ok(SyscallErrorCode::kContractAlreadyDeployed),
            e if e == SyscallErrorCode::kNoReturnBufferAvailable as i32 =>
                Ok(SyscallErrorCode::kNoReturnBufferAvailable),
            e if e == SyscallErrorCode::kInsufficientGasForInvocation as i32 =>
                Ok(SyscallErrorCode::kInsufficientGasForInvocation),
            e if e == SyscallErrorCode::kInvalidSyscallInput as i32 =>
                Ok(SyscallErrorCode::kInvalidSyscallInput),
            e if e == SyscallErrorCode::kStorageKeyNotExist as i32 =>
                Ok(SyscallErrorCode::kStorageKeyNotExist),
            e if e == SyscallErrorCode::kCalldataParseError as i32 =>
                Ok(SyscallErrorCode::kCalldataParseError),
            e if e == SyscallErrorCode::kReturnParseError as i32 =>
                Ok(SyscallErrorCode::kReturnParseError),
            e if e == SyscallErrorCode::kDeterministicWasm as i32 =>
                Ok(SyscallErrorCode::kDeterministicWasm),
            e => Err(e)
        }
    }
}
