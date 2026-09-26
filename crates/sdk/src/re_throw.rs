use crate::exn::{LibraryExn, syscall_exn, syscall_throw};
use crate::sys::{SysErr, SyscallNo};

/// A type that can re-throw itself as a contract exception or fault.
pub trait ReThrow {
    /// Re-throw this error. Exceptions are propagated as exceptions;
    /// faults panic (a fault stays a fault).
    #[cfg_attr(not(target_arch = "riscv32"), track_caller)]
    fn re_throw(self) -> !;
}

/// Extension trait for transparently propagating a contract error.
pub trait OrReThrow<T> {
    /// On `Err`, re-throw the error. On `Ok`, return the value.
    fn or_re_throw(self) -> T;
}

impl<T, E: ReThrow> OrReThrow<T> for Result<T, E> {
    #[cfg_attr(not(target_arch = "riscv32"), track_caller)]
    fn or_re_throw(self) -> T {
        match self {
            Ok(v) => v,
            Err(e) => e.re_throw(),
        }
    }
}

impl ReThrow for core::convert::Infallible {
    #[cfg_attr(not(target_arch = "riscv32"), track_caller)]
    fn re_throw(self) -> ! {
        match self {}
    }
}

/// Extension trait for turning a failed raw syscall into the SDK's syscall
/// exception. A [`SysErr`] is only the error code, so the caller names the
/// syscall; the exception carries both (see
/// [`SyscallExn`](crate::exn::SyscallExn)).
pub trait OrSyscallThrow<T> {
    /// On `Err`, throw the syscall exception for `callno` (see
    /// [`syscall_throw`]). On `Ok`, return the value.
    fn or_syscall_throw(self, callno: SyscallNo) -> T;

    /// On `Err`, convert to the [`LibraryExn`] that
    /// [`or_syscall_throw`](Self::or_syscall_throw) would throw (see
    /// [`syscall_exn`]).
    fn or_syscall_exn(self, callno: SyscallNo) -> Result<T, LibraryExn>;
}

impl<T> OrSyscallThrow<T> for Result<T, SysErr> {
    // A `match`, not `unwrap_or_else`: a closure is not `track_caller`, so
    // the throw would report this line instead of the caller's.
    #[cfg_attr(not(target_arch = "riscv32"), track_caller)]
    fn or_syscall_throw(self, callno: SyscallNo) -> T {
        match self {
            Ok(v) => v,
            Err(errno) => syscall_throw(callno, errno),
        }
    }

    fn or_syscall_exn(self, callno: SyscallNo) -> Result<T, LibraryExn> {
        self.map_err(|errno| syscall_exn(callno, errno))
    }
}
