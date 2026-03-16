//! System function: Delete

use crate::sys::{self, SysErr};
use crate::types::StorageKey;
use groundhog_xdr::storage_item::*;

pub fn try_delete_shared(key: &StorageKey) -> Result<(), SysErr> {
    let flags = StorageDeltaFlags::kStorageDeltaFlagNone;

    sys::raw_DeleteObject(key, flags.0)
}

pub fn delete_shared(key: &StorageKey) {
    try_delete_shared(key).unwrap_or_else(|errno| {
        crate::exn::syscall_abort(crate::sys::SyscallNo::kDeleteObject, errno)
    });
}

pub fn try_delete_exclusive(key: &StorageKey) -> Result<(), SysErr> {
    let flags = StorageDeltaFlags::kStorageDeltaFlagExclusiveLock;

    sys::raw_DeleteObject(key, flags.0)
}

pub fn delete_exclusive(key: &StorageKey) {
    try_delete_exclusive(key).unwrap_or_else(|errno| {
        crate::exn::syscall_abort(crate::sys::SyscallNo::kDeleteObject, errno)
    });
}

/// Delete anything stored at the given key.
///
/// Succeeds no matter what type of item is stored there, or if none is. After
/// calling this function for a particular key, the storage location behaves as
/// if it had never been set.  An item of any type may then be stored there.
pub fn delete(key: &StorageKey, exclusive: bool) {
    if exclusive {
        delete_exclusive(key)
    } else {
        delete_shared(key)
    }
}
