// -*-Rust-*-

use core::convert::TryInto;

use super::{SyscallErrorCode, SysErr};

//impl TryInto<SyscallErrorCode> for SysErr {
//    type Error = i32;
//
//    fn try_into(self) -> Result<SyscallErrorCode, Self::Error> {
//        match self.0 {
//            // syscall error code match lines
//             e => Err(e)
//        }
//    }
//}

macros::generate_syscall_error_helper!("protocol-rs/protocol/syscalls.yaml");
