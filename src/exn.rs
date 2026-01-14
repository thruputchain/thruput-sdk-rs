use crate::sys::{SysErr, SyscallNo};
use core::fmt::{self, Display};
use crate::{ReadXDR, WriteXDR};

#[derive(Debug, Clone, Copy)]
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

// temporary hack until we can get the XDR impls

#[derive(Debug, ReadXDR, WriteXDR)]
pub struct SyscallExn2 {
    callno: i32,
    errno: SysErr,
}

impl From<SyscallExn> for SyscallExn2 {
    fn from(e: SyscallExn) -> Self {
        Self {
            callno: e.callno as i32,
            errno: e.errno
        }
    }
}
