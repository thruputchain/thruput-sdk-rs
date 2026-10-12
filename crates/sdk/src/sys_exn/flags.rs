//! Storage flags: the locks a transaction requests for the keys it writes.
//!
//! These are the throwing forms of
//! [`sys_result::flags`](crate::sys_result::flags), which describes how flags
//! behave. Leaf storage handles expose the same operations through
//! [`Flaggable`](crate::storage::Flaggable).

use crate::re_throw::OrSyscallThrow;
use crate::sys::SyscallNo;
use crate::sys_result::flags as result;
use crate::types::StorageKey;
use thruput_xdr::storage_item::StorageDeltaFlags;

/// Returns the flags the current transaction requests for `key`, as seen by
/// the current call frame. A key with no pending write has none.
pub fn storage_flags(key: &StorageKey) -> StorageDeltaFlags {
    result::storage_flags(key).or_syscall_throw(SyscallNo::kGetStorageFlags)
}

/// Adds `flags` (a bitwise OR) to the flags the current transaction requests
/// for `key`.
///
/// Throws a syscall exception if no write to `key` is pending in the current
/// call frame, or if `flags` holds an unknown flag. See
/// [`sys_result::flags::set_storage_flags`](crate::sys_result::flags::set_storage_flags).
pub fn set_storage_flags(key: &StorageKey, flags: StorageDeltaFlags) {
    result::set_storage_flags(key, flags)
        .or_syscall_throw(SyscallNo::kSetStorageFlags);
}

/// Removes every flag the current transaction requests for `key`. Does
/// nothing if no write to `key` is pending.
pub fn clear_storage_flags(key: &StorageKey) {
    result::clear_storage_flags(key)
        .or_syscall_throw(SyscallNo::kClearStorageFlags);
}

/// Requests an exclusive lock on `key` after a write to it succeeded, if the
/// write's policy is exclusive; the one place a write policy reaches the host.
/// A shared write adds no syscall.
///
/// The policy is a `bool`, not a flag set, so the only flag this can request
/// is `ExclusiveLock`, and the write just left a pending write in the current
/// frame: the setter cannot fail here. If it fails anyway, this throws, which
/// reverts the frame and the write with it instead of leaving the write in
/// place without its lock. Arbitrary flags go through [`set_storage_flags`],
/// which writes nothing, so a rejected flag set leaves storage unchanged.
pub(crate) fn lock_after_write(key: &StorageKey, exclusive: bool) {
    if exclusive {
        set_storage_flags(key, StorageDeltaFlags::ExclusiveLock);
    }
}
