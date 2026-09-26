//! System function: Delete

use crate::re_throw::OrSyscallThrow;
use crate::sys_result::delete as result;
use crate::types::StorageKey;
use groundhog_xdr::storage_item::StorageDeltaFlags;

pub fn delete_shared(key: &StorageKey) {
    result::delete_shared(key)
        .or_syscall_throw(crate::sys::SyscallNo::kDeleteObject);
}

pub fn delete_exclusive(key: &StorageKey) {
    result::delete_exclusive(key)
        .or_syscall_throw(crate::sys::SyscallNo::kDeleteObject);
}

pub fn delete_with_flags(key: &StorageKey, flags: StorageDeltaFlags) {
    result::delete_with_flags(key, flags)
        .or_syscall_throw(crate::sys::SyscallNo::kDeleteObject);
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
