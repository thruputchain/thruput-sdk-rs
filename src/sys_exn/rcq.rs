use crate::sys::SyscallNo;
use crate::sys_result::rcq as rcq_result;
use crate::syscall_throw;
use crate::types::StorageKey;

#[cfg(feature = "alloc")]
use crate::{Box, sys::SyscallErrorCode};

pub fn retrocausal_queue_insert(key: &StorageKey, queue_entry: &[u8]) {
    rcq_result::retrocausal_queue_insert(key, queue_entry).unwrap_or_else(
        |errno| syscall_throw(SyscallNo::kRetrocausalQueueInsert, errno),
    )
}

pub fn retrocausal_queue_consume_sized<const N: usize>(
    key: &StorageKey,
) -> [u8; N] {
    rcq_result::retrocausal_queue_get_sized(key, false).unwrap_or_else(
        |errno| syscall_throw(SyscallNo::kRetrocausalQueueConsume, errno),
    )
}

pub fn retrocausal_queue_try_consume_sized<const N: usize>(
    key: &StorageKey,
) -> Option<[u8; N]> {
    rcq_result::retrocausal_queue_try_get_sized(key, false).unwrap_or_else(
        |errno| syscall_throw(SyscallNo::kRetrocausalQueueConsume, errno),
    )
}

pub fn retrocausal_queue_peek_sized<const N: usize>(
    key: &StorageKey,
) -> [u8; N] {
    rcq_result::retrocausal_queue_get_sized(key, true).unwrap_or_else(
        |errno| syscall_throw(SyscallNo::kRetrocausalQueueConsume, errno),
    )
}

pub fn retrocausal_queue_try_peek_sized<const N: usize>(
    key: &StorageKey,
) -> Option<[u8; N]> {
    rcq_result::retrocausal_queue_try_get_sized(key, true).unwrap_or_else(
        |errno| syscall_throw(SyscallNo::kRetrocausalQueueConsume, errno),
    )
}

#[cfg(feature = "alloc")]
pub fn retrocausal_queue_consume(key: &StorageKey) -> Box<[u8]> {
    retrocausal_queue_get(key, false)
}

#[cfg(feature = "alloc")]
pub fn retrocausal_queue_try_consume(key: &StorageKey) -> Option<Box<[u8]>> {
    retrocausal_queue_try_get(key, false)
}

#[cfg(feature = "alloc")]
pub fn retrocausal_queue_peek(key: &StorageKey) -> Box<[u8]> {
    retrocausal_queue_get(key, true)
}

#[cfg(feature = "alloc")]
pub fn retrocausal_queue_try_peek(key: &StorageKey) -> Option<Box<[u8]>> {
    retrocausal_queue_try_get(key, true)
}

/// Get the next queue entry's data into a boxed slice.
///
/// If `peek` is true, the entry will not be consumed.
///
#[cfg(feature = "alloc")]
pub(crate) fn retrocausal_queue_get(
    key: &StorageKey,
    peek: bool,
) -> Box<[u8]> {
    rcq_result::retrocausal_queue_get(key, peek).unwrap_or_else(
        // XXX: We don't actually know which syscall failed.
        // The result-based routines should return syscall exns.
        |errno| syscall_throw(SyscallNo::kRetrocausalQueueConsume, errno),
    )
}

/// Get the next queue entry's data into a boxed slice, if present.
///
/// If the queue is empty, returns `None`.
/// If `peek` is true, the entry will not be consumed.
///
#[cfg(feature = "alloc")]
pub(crate) fn retrocausal_queue_try_get(
    key: &StorageKey,
    peek: bool,
) -> Option<Box<[u8]>> {
    match rcq_result::retrocausal_queue_get(key, peek) {
        Ok(v) => Some(v),
        Err(e)
            if e.is_code(
                SyscallErrorCode::kSyscallErrorCodeNoRetrocausalQueueEntry,
            ) =>
        {
            None
        }
        Err(errno) => {
            syscall_throw(SyscallNo::kRetrocausalQueueConsume, errno)
        }
    }
}
