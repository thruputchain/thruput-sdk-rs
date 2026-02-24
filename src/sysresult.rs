// For whose only error is a SysErr (syscalls or syscall-wrappers)
pub type SysResult<T> = core::result::Result<T, crate::SysErr>;
