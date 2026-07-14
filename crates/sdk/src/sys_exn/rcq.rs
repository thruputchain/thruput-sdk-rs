use crate::exn::{LibraryExn, syscall_exn};
use crate::internal::ScratchBuf;
use crate::sys::SyscallNo;
use crate::sys_result::rcq as rcq_result;
use crate::syscall_throw;
use crate::types::StorageKey;

#[cfg(feature = "alloc")]
use alloc::boxed::Box;

// Every rcq error below is attributed to `kRetrocausalQueueConsume`, even
// when the failing syscall was `RetrocausalQueuePeekLen` issued on its
// behalf — the result-layer routines don't report which call failed, and
// the historical thrown exceptions already used the consume number.

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

/// Consume the next queue entry as an `N`-byte array, reporting failure as
/// `Err` instead of aborting (the `try_` law; see the
/// [`exn`](mod@crate::exn) module docs).
///
/// An empty queue is a `LibraryExn::Syscall` for which
/// [`is_storage_missing`](LibraryExn::is_storage_missing) is true; an entry
/// of a different size is `LibraryExn::RetrocausalQueueEntryWrongSize` and is left in
/// the queue (the size is checked by peeking before consuming).
pub fn retrocausal_queue_try_consume_sized<const N: usize>(
    key: &StorageKey,
) -> Result<[u8; N], LibraryExn> {
    retrocausal_queue_try_sized(key, false)
}

pub fn retrocausal_queue_peek_sized<const N: usize>(
    key: &StorageKey,
) -> [u8; N] {
    rcq_result::retrocausal_queue_get_sized(key, true).unwrap_or_else(
        |errno| syscall_throw(SyscallNo::kRetrocausalQueueConsume, errno),
    )
}

/// Peek at the next queue entry as an `N`-byte array; the `try_`-law
/// counterpart of [`retrocausal_queue_peek_sized`]. Same error contract as
/// [`retrocausal_queue_try_consume_sized`].
pub fn retrocausal_queue_try_peek_sized<const N: usize>(
    key: &StorageKey,
) -> Result<[u8; N], LibraryExn> {
    retrocausal_queue_try_sized(key, true)
}

/// Shared body of the sized `try_` forms. Checks the entry's length by
/// peeking before reading it, so a wrong-size entry is reported without
/// being consumed (and without tripping the read path's size assertion).
fn retrocausal_queue_try_sized<const N: usize>(
    key: &StorageKey,
    peek: bool,
) -> Result<[u8; N], LibraryExn> {
    let found = rcq_result::retrocausal_queue_peek_len(key)
        .map_err(|e| syscall_exn(SyscallNo::kRetrocausalQueueConsume, e))?;
    if found != N as u32 {
        return Err(LibraryExn::RetrocausalQueueEntryWrongSize {
            expected: N as u32,
            found,
        });
    }
    rcq_result::retrocausal_queue_get_sized(key, peek)
        .map_err(|e| syscall_exn(SyscallNo::kRetrocausalQueueConsume, e))
}

/// Read the next queue entry into `buf` (cleared first), reporting every
/// failure as `Err`
/// (the `try_` law): an empty queue is the
/// [`is_storage_missing`](LibraryExn::is_storage_missing) syscall exn, an
/// entry larger than `buf` is `StorageSerialization` (unreachable for the
/// SDK scratch, which is larger than the protocol's entry cap), and any
/// other syscall failure its syscall exn. With `peek == false` the
/// entry-larger-than-`buf` error is destructive (see
/// [`rcq_result::retrocausal_queue_read_into`]); `try_` callers that must
/// leave a bad entry in the queue read with `peek == true` and pop via
/// [`retrocausal_queue_try_discard`].
pub(crate) fn retrocausal_queue_try_read_into(
    key: &StorageKey,
    peek: bool,
    buf: &mut ScratchBuf,
) -> Result<(), LibraryExn> {
    match rcq_result::retrocausal_queue_read_into(key, peek, buf) {
        Ok(Some(())) => Ok(()),
        Ok(None) => Err(LibraryExn::StorageSerialization),
        Err(e) => Err(syscall_exn(SyscallNo::kRetrocausalQueueConsume, e)),
    }
}

/// Pop the next queue entry without copying its contents, reporting failure
/// as `Err`. Completes a typed `try_consume` after the entry has been peeked
/// and decoded, so the pop can no longer fail on the entry's contents.
pub(crate) fn retrocausal_queue_try_discard(
    key: &StorageKey,
) -> Result<(), LibraryExn> {
    rcq_result::retrocausal_queue_consume_discard(key)
        .map_err(|e| syscall_exn(SyscallNo::kRetrocausalQueueConsume, e))
}

#[cfg(feature = "alloc")]
pub fn retrocausal_queue_consume(key: &StorageKey) -> Box<[u8]> {
    use crate::re_throw::OrReThrow;
    retrocausal_queue_try_consume(key).or_re_throw()
}

/// Consume the next queue entry into a boxed slice, reporting failure as
/// `Err` instead of aborting (the `try_` law). An empty queue is a
/// `LibraryExn::Syscall` for which
/// [`is_storage_missing`](LibraryExn::is_storage_missing) is true.
#[cfg(feature = "alloc")]
pub fn retrocausal_queue_try_consume(
    key: &StorageKey,
) -> Result<Box<[u8]>, LibraryExn> {
    retrocausal_queue_try_get(key, false)
}

#[cfg(feature = "alloc")]
pub fn retrocausal_queue_peek(key: &StorageKey) -> Box<[u8]> {
    use crate::re_throw::OrReThrow;
    retrocausal_queue_try_peek(key).or_re_throw()
}

/// Peek at the next queue entry as a boxed slice; the `try_`-law
/// counterpart of [`retrocausal_queue_peek`]. Same error contract as
/// [`retrocausal_queue_try_consume`].
#[cfg(feature = "alloc")]
pub fn retrocausal_queue_try_peek(
    key: &StorageKey,
) -> Result<Box<[u8]>, LibraryExn> {
    retrocausal_queue_try_get(key, true)
}

/// Get the next queue entry's data into a boxed slice.
///
/// If `peek` is true, the entry will not be consumed.
///
#[cfg(feature = "alloc")]
fn retrocausal_queue_try_get(
    key: &StorageKey,
    peek: bool,
) -> Result<Box<[u8]>, LibraryExn> {
    rcq_result::retrocausal_queue_get(key, peek)
        .map_err(|e| syscall_exn(SyscallNo::kRetrocausalQueueConsume, e))
}
