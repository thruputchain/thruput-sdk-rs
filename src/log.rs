//! System functions: logging

use crate::sysresult::SysResult as Result;
use crate::sys;

pub fn log(buffer: &[u8]) -> Result<()> {
    sys::raw_ConsoleLog(buffer)
}

#[cfg(feature = "mock")]
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
