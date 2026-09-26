//! Storage flags: the locks a transaction requests for the keys it writes.
//!
//! A key's flags belong to the transaction's pending write to that key, not to
//! any single write. Writes never change them: [`set_storage_flags`] adds
//! flags and [`clear_storage_flags`] removes them all. `ExclusiveLock` is the
//! only flag today.
//!
//! Flags follow frames: changes made in a frame that aborts are discarded with
//! that frame's writes.
//!
//! These routines are thin wrappers around VM syscalls. They return
//! [`Result<T, E>`](core::result::Result) where `E` is a
//! [`SysErr`](crate::sys::SysErr) containing an error code in the case of a
//! syscall failure.

use crate::exn::syscall_throw;
use crate::sys::{self, SysResult as Result, SyscallNo};
use crate::types::StorageKey;
use groundhog_xdr::storage_item::StorageDeltaFlags;

/// Returns the flags the current transaction requests for `key`, as seen by
/// the current call frame. A key with no pending write has none.
pub fn storage_flags(key: &StorageKey) -> Result<StorageDeltaFlags> {
    let mut out = StorageDeltaFlags::None.0;
    sys::raw_GetStorageFlags(key, &mut out)?;
    Ok(StorageDeltaFlags(out))
}

/// Adds `flags` (a bitwise OR) to the flags the current transaction requests
/// for `key`.
///
/// A write to `key` must be pending in the current call frame: one made
/// earlier in this frame, or one inherited from a caller or a successful
/// callee. A pending deletion qualifies; a value that only exists from an
/// earlier block does not. Otherwise, or if `flags` holds an unknown flag,
/// this returns
/// [`kSyscallErrorCodeInvalidSyscallInput`](sys::SyscallErrorCode::kSyscallErrorCodeInvalidSyscallInput).
pub fn set_storage_flags(
    key: &StorageKey,
    flags: StorageDeltaFlags,
) -> Result {
    sys::raw_SetStorageFlags(key, flags.0)
}

/// Removes every flag the current transaction requests for `key`, as seen by
/// the current call frame. Does nothing if no write to `key` is pending.
pub fn clear_storage_flags(key: &StorageKey) -> Result {
    sys::raw_ClearStorageFlags(key)
}

/// Requests `flags` for `key` after a write to it succeeded; the one place a
/// write policy's flags reach the host. `None` adds no syscall.
///
/// The write left a pending write in the current frame, so the setter cannot
/// fail for a known flag. If it fails anyway, this throws: that reverts the
/// frame and the write with it, instead of leaving the write in place without
/// the lock it asked for.
pub(crate) fn request_after_write(key: &StorageKey, flags: StorageDeltaFlags) {
    if flags.0 == StorageDeltaFlags::None.0 {
        return;
    }
    if let Err(errno) = set_storage_flags(key, flags) {
        syscall_throw(SyscallNo::kSetStorageFlags, errno);
    }
}
