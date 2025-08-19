use thiserror::Error;

pub type SyscallResult<T> = core::result::Result<T, SysErr>;

/// Error type for syscalls
#[derive(serde::Serialize, serde::Deserialize)]
#[derive(xdrs::ReadXDR, xdrs::WriteXDR)]
#[cfg_attr(feature = "spec", derive(xdrs::XDR))]
#[derive(Error, Copy, Clone, Debug, PartialEq, Eq)]
#[error("SysErr({0})")]
pub struct SysErr(pub i32);

/// Error codes
pub use groundhog_xdr::syscall_error::SyscallErrorCode;

impl SysErr {
    pub fn is_code(self, code: SyscallErrorCode) -> bool {
        self.0 == code as i32
        /* if let Ok(c) = TryInto::<SyscallErrorCode>::try_into(self.0) {
            c == code
        } else {
            false
        } */
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
        self.is_code(SyscallErrorCode::kFrameReverted)
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
