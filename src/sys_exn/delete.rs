//! System function: Delete

use crate::sys_result::delete as result;
use crate::types::StorageKey;

pub fn delete_shared(key: &StorageKey) {
    result::delete_shared(key).unwrap_or_else(|errno| {
        crate::exn::syscall_throw(crate::sys::SyscallNo::kDeleteObject, errno)
    });
}

pub fn delete_exclusive(key: &StorageKey) {
    result::delete_exclusive(key).unwrap_or_else(|errno| {
        crate::exn::syscall_throw(crate::sys::SyscallNo::kDeleteObject, errno)
    });
}

/// Delete anything stored at the given key.
///
/// Succeeds no matter what type of item is stored there, or if none is. After
/// calling this function for a particular key, the storage location behaves as
/// if it had never been set.
///
pub fn delete(key: &StorageKey, exclusive: bool) {
    if exclusive {
        delete_exclusive(key)
    } else {
        delete_shared(key)
    }
}
