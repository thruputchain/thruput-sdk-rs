//! Storage flags: the locks a transaction requests for the keys it writes.
//!
//! These are the throwing forms of
//! [`sys_result::flags`](crate::sys_result::flags), which describes how flags
//! behave. Leaf storage handles expose the same operations through
//! [`Flaggable`](crate::storage::Flaggable).

use crate::exn::syscall_throw;
use crate::sys::SyscallNo;
use crate::sys_result::flags as result;
use crate::types::StorageKey;
use groundhog_xdr::storage_item::StorageDeltaFlags;

/// Returns the flags the current transaction requests for `key`, as seen by
/// the current call frame. A key with no pending write has none.
pub fn storage_flags(key: &StorageKey) -> StorageDeltaFlags {
    result::storage_flags(key).unwrap_or_else(|errno| {
        syscall_throw(SyscallNo::kGetStorageFlags, errno)
    })
}

/// Adds `flags` (a bitwise OR) to the flags the current transaction requests
/// for `key`.
///
/// Throws a syscall exception if no write to `key` is pending in the current
/// call frame, or if `flags` holds an unknown flag. See
/// [`sys_result::flags::set_storage_flags`](crate::sys_result::flags::set_storage_flags).
pub fn set_storage_flags(key: &StorageKey, flags: StorageDeltaFlags) {
    result::set_storage_flags(key, flags).unwrap_or_else(|errno| {
        syscall_throw(SyscallNo::kSetStorageFlags, errno)
    });
}

/// Removes every flag the current transaction requests for `key`. Does
/// nothing if no write to `key` is pending.
pub fn clear_storage_flags(key: &StorageKey) {
    result::clear_storage_flags(key).unwrap_or_else(|errno| {
        syscall_throw(SyscallNo::kClearStorageFlags, errno)
    });
}

/// Requests `flags` for `key` after a write to it succeeded; the one place a
/// write policy's flags reach the host. `None` adds no syscall.
///
/// The write left a pending write in the current frame, so the setter cannot
/// fail for a known flag. If it fails anyway, this throws: that reverts the
/// frame and the write with it, instead of leaving the write in place without
/// the lock it asked for.
pub(crate) fn request_after_write(key: &StorageKey, flags: StorageDeltaFlags) {
    if flags != StorageDeltaFlags::None {
        set_storage_flags(key, flags);
    }
}
