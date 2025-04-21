// -*-Rust-*-

use core::convert::TryInto;

use super::{SyscallErrorCode, SysErr};

macros::generate_syscall_error_helper!("protocol-rs/protocol/syscalls.yaml");
