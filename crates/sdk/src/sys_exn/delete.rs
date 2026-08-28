//! System function: Delete

use crate::sys_result::delete as result;
use crate::types::StorageKey;
use groundhog_xdr::storage_item::StorageDeltaFlags;

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

pub fn delete_with_flags(key: &StorageKey, flags: StorageDeltaFlags) {
    result::delete_with_flags(key, flags).unwrap_or_else(|errno| {
        crate::exn::syscall_throw(crate::sys::SyscallNo::kDeleteObject, errno)
    });
}

/// Delete anything stored at the given key.
///
/// Succeeds no matter what type of item is stored there, or if none is. After
/// calling this function for a particular key, the storage location behaves as
/// if it had never been set.
///
/// One exception: a retrocausal queue whose retained history spans more than
/// `kMaxRetrocausalQueueBlocksPerDeletion` live block rows cannot be deleted
/// in one transaction. The syscall returns
/// `kSyscallErrorCodeRetrocausalQueueTooLargeToDelete` (thrown here; surfaced
/// as an `Err` by the `sys_result` variants), a handleable resource limit:
/// consume the queue down in earlier blocks and retry.
///
pub fn delete(key: &StorageKey, exclusive: bool) {
    if exclusive {
        delete_exclusive(key)
    } else {
        delete_shared(key)
    }
}
