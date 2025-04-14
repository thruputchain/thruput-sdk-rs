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
