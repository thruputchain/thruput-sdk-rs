#![allow(non_snake_case)]

use crate::result::SysResult as Result;
use crate::sys;

pub fn log(buffer: &[u8]) -> Result<()> {
    sys::raw_Log(buffer)
}

#[cfg(feature = "mock")]
pub fn log_str<S: AsRef<str>>(msg: S) {
    std::println!("::LOG:: {}", msg.as_ref());
}

#[cfg(feature = "mock")]
#[macro_export] 
macro_rules! debug_log {
    ($($arg:tt)*) => {
        std::println!($($arg)*);
    };
}

#[cfg(not(feature = "mock"))]
#[macro_export]
macro_rules! debug_log {
    ($($arg:tt)*) => {};
}

