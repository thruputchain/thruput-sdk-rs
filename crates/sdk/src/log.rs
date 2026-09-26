//! System functions: logging

use crate::re_throw::OrSyscallThrow;
use crate::sys;

pub fn log(buffer: &[u8]) {
    sys::raw_ConsoleLog(buffer)
        .or_syscall_throw(crate::sys::SyscallNo::kConsoleLog);
}

#[cfg(feature = "dev")]
pub fn log_str<S: AsRef<str>>(msg: S) {
    std::println!("::LOG:: {}", msg.as_ref());
}

#[cfg(feature = "std")]
#[macro_export]
macro_rules! debug_log {
    ($($arg:tt)*) => {
        std::println!($($arg)*);
    };
}

#[cfg(not(feature = "std"))]
#[macro_export]
macro_rules! debug_log {
    ($($arg:tt)*) => {};
}
