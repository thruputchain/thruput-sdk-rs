use crate::sys::{SysErr, SyscallNo};
use core::fmt::{self, Display};

#[derive(Debug)]
pub struct SyscallExn {
    callno: SyscallNo,
    errno: SysErr,
}

impl SyscallExn {
    pub fn new(
        callno: SyscallNo,
        errno: SysErr,
    ) -> Self {
        Self {
            callno,
            errno,
        }
    }
}

impl Display for SyscallExn {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Syscall {:?} failed with {}", self.callno, self.errno)
    }
}
