/// Rust bindings for the syscalls
pub mod bindings;

// provides TryInto<SyscallErrorCode> for SysErr
#[rustfmt::skip]
mod error_helper;

use ::xdrs::{ReadXDR, WriteXDR};
use thiserror::Error;

#[derive(Error, Copy, Clone, Debug, PartialEq, Eq)]
#[derive(serde::Serialize, serde::Deserialize)]
#[derive(ReadXDR, WriteXDR)]
#[error("SysErr({0})")]
pub struct SysErr(i32);

pub use bindings::SyscallErrorCode;

impl SysErr {
    pub fn is_code(self, code: SyscallErrorCode) -> bool {
        if let Ok(c) = TryInto::<SyscallErrorCode>::try_into(self) {
            c == code
        } else {
            false
        }
    }

    pub fn is_none(self) -> bool {
        self.is_code(SyscallErrorCode::kNone)
    }

    pub fn is_some(self) -> bool {
        !self.is_none()
    }

    pub fn is_storage_key_not_exist(self) -> bool {
        self.is_code(SyscallErrorCode::kStorageKeyNotExist)
    }

    pub fn is_aborted(self) -> bool {
        // XXX
        false
    }
}

impl From<SyscallErrorCode> for SysErr {
    fn from(code: SyscallErrorCode) -> Self {
        SysErr(code as i32)
    }
}

impl From<SysErr> for i32 {
    fn from(err: SysErr) -> Self {
        err.0
    }
}

pub type SyscallResult<T> = core::result::Result<T, SysErr>;
