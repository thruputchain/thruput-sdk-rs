#![allow(dead_code)]

/// Rust bindings for the syscalls
#[rustfmt::skip]
pub mod bindings;

// provides TryInto<SyscallErrorCode> for SysErr
#[rustfmt::skip]
mod error_helper;

pub(crate) fn to_offset_mut<T>(obj: &mut T) -> *mut u8 {
    obj as *mut T as *mut u8
}

pub(crate) fn to_offset<T>(obj: &T) -> *const u8 {
    obj as *const T as *const u8
}
