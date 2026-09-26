//! Storage flags: the locks a transaction requests for the keys it writes.
//!
//! A key's flags belong to the transaction's pending write to that key, not to
//! any single write. Writes never change them: [`set_storage_flags`] adds
//! flags, [`clear_storage_flags`] removes them all, and whatever the key's
//! flags are when the transaction commits (or when a `TryInvoke` call pushes
//! its changes) go with the key's delta. `ExclusiveLock` is the only flag
//! today.
//!
//! Flags follow frames: those set in a frame that reverts, or in a
//! `TryInvoke` call that fails, are discarded with that frame's writes.
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

/// Removes every flag the current transaction requests for `key`. Does
/// nothing if no write to `key` is pending.
///
/// If a `TryInvoke` call already pushed the flags, the transaction's next push
/// withdraws the post-commit freeze they asked for; the key stays locked
/// against other transactions until this one commits.
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
