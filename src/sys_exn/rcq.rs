use crate::sys::{SyscallErrorCode, SyscallNo};
use crate::sys_result::rcq as rcq_result;
use crate::syscall_throw;
use crate::types::StorageKey;

pub fn retrocausal_queue_insert(key: &StorageKey, queue_entry: &[u8]) {
    rcq_result::retrocausal_queue_insert(key, queue_entry).unwrap_or_else(
        |errno| syscall_throw(SyscallNo::kRetrocausalQueueInsert, errno),
    )
}

pub fn retrocausal_queue_consume_sized<const N: usize>(
    key: &StorageKey,
) -> [u8; N] {
    rcq_result::retrocausal_queue_consume_sized(key).unwrap_or_else(|errno| {
        syscall_throw(SyscallNo::kRetrocausalQueueConsume, errno)
    })
}

pub fn retrocausal_queue_try_consume_sized<const N: usize>(
    key: &StorageKey,
) -> Option<[u8; N]> {
    match rcq_result::retrocausal_queue_consume_sized(key) {
        Ok(v) => Some(v),
        Err(e)
            if e.is_code(
                SyscallErrorCode::kSyscallErrorCodeNoRetrocausalQueueEntry,
            ) =>
        {
            None
        }
        Err(e) => syscall_throw(SyscallNo::kRetrocausalQueueConsume, e),
    }
}

pub fn retrocausal_queue_peek_sized<const N: usize>(
    key: &StorageKey,
) -> [u8; N] {
    rcq_result::retrocausal_queue_peek_sized(key).unwrap_or_else(|errno| {
        syscall_throw(SyscallNo::kRetrocausalQueueConsume, errno)
    })
}

/// Get the next queue entry's data.
///
/// A vector of maximum length `N` is returned.
/// If `peek` is true, the entry will not be consumed.
///
#[cfg(feature = "alloc")]
pub fn retrocausal_queue_get_max<const N: usize>(
    key: &StorageKey,
    peek: bool,
) -> crate::Vec<u8> {
    rcq_result::retrocausal_queue_get_max::<N>(key, peek).unwrap_or_else(
        |errno| syscall_throw(SyscallNo::kRetrocausalQueueConsume, errno),
    )
}
