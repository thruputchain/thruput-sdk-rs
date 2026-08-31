//! System functions: logging

use crate::sys;

pub fn log(buffer: &[u8]) {
    sys::raw_ConsoleLog(buffer).unwrap_or_else(|errno| {
        crate::syscall_throw(crate::sys::SyscallNo::kConsoleLog, errno)
    });
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
