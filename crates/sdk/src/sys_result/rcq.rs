//! System functions: retro-causal queues

use crate::internal::ScratchBuf;
use crate::sys;
use crate::sys::SysResult as Result;
use crate::sys::SyscallErrorCode;
use crate::types::{RuntimeMemoryLength, StorageKey};

use core::mem::MaybeUninit;

pub fn retrocausal_queue_insert(
    key: &StorageKey,
    queue_entry: &[u8],
) -> Result<()> {
    sys::raw_RetrocausalQueueInsert(key, queue_entry)
}

pub fn retrocausal_queue_consume_sized<const N: usize>(
    key: &StorageKey,
) -> Result<[u8; N]> {
    let peek = false;

    retrocausal_queue_get_sized(key, peek)
}

pub fn retrocausal_queue_peek_sized<const N: usize>(
    key: &StorageKey,
) -> Result<[u8; N]> {
    let peek = true;

    retrocausal_queue_get_sized(key, peek)
}

pub fn retrocausal_queue_peek_len(key: &StorageKey) -> Result<u32> {
    let mut len: MaybeUninit<RuntimeMemoryLength> = MaybeUninit::uninit();

    sys::uninit_RetrocausalQueuePeekLen(key, &mut len)?;

    Ok(unsafe { len.assume_init() })
}

pub(crate) fn retrocausal_queue_get_sized<const N: usize>(
    key: &StorageKey,
    peek: bool,
) -> Result<[u8; N]> {
    let mut queue_entry = [MaybeUninit::uninit(); N];
    let mut written_len: MaybeUninit<RuntimeMemoryLength> =
        MaybeUninit::uninit();
    sys::uninit_RetrocausalQueueConsume(
        key,
        peek,
        &mut queue_entry,
        &mut written_len,
    )?;

    let written_len = unsafe { written_len.assume_init() };

    assert_eq!(written_len, N as u32);

    let bytes: [u8; N] =
        unsafe { queue_entry.map(|elt| MaybeUninit::assume_init(elt)) };

    Ok(bytes)
}

pub(crate) fn retrocausal_queue_try_get_sized<const N: usize>(
    key: &StorageKey,
    peek: bool,
) -> Result<Option<[u8; N]>> {
    match retrocausal_queue_get_sized(key, peek) {
        Ok(v) => Ok(Some(v)),
        Err(e)
            if e.is_code(
                SyscallErrorCode::kSyscallErrorCodeNoRetrocausalQueueEntry,
            ) =>
        {
            Ok(None)
        }
        Err(e) => Err(e),
    }
}

/// Read the next queue entry into `buf` (cleared first).
///
/// Returns `Ok(None)` if the entry exceeds `buf`'s capacity (only possible in
/// the no-alloc guest; entries written via the typed `RCQ<T>` insert path
/// always fit). Filling a caller-provided buffer lets the typed read paths
/// use the reused in-place scratch.
pub(crate) fn retrocausal_queue_read_into(
    key: &StorageKey,
    peek: bool,
    buf: &mut ScratchBuf,
) -> Result<Option<()>> {
    let len = retrocausal_queue_peek_len(key)? as usize;
    buf.clear();
    #[cfg(feature = "alloc")]
    buf.reserve(len);
    if len > buf.spare_capacity_mut().len() {
        return Ok(None);
    }
    let mut written_len = MaybeUninit::uninit();
    sys::uninit_RetrocausalQueueConsume(
        key,
        peek,
        buf.spare_capacity_mut(),
        &mut written_len,
    )?;
    debug_assert!(unsafe { written_len.assume_init() } as usize == len);
    unsafe { buf.set_len(len) };
    Ok(Some(()))
}

/// Like [`retrocausal_queue_read_into`] but distinguishes an empty queue.
///
/// `Ok(None)` = queue empty; `Ok(Some(None))` = entry too large for `buf`;
/// `Ok(Some(Some(())))` = read into `buf`.
pub(crate) fn retrocausal_queue_try_read_into(
    key: &StorageKey,
    peek: bool,
    buf: &mut ScratchBuf,
) -> Result<Option<Option<()>>> {
    match retrocausal_queue_read_into(key, peek, buf) {
        Ok(v) => Ok(Some(v)),
        Err(e)
            if e.is_code(
                SyscallErrorCode::kSyscallErrorCodeNoRetrocausalQueueEntry,
            ) =>
        {
            Ok(None)
        }
        Err(e) => Err(e),
    }
}

/// Get the next queue entry into a boxed slice.
///
#[cfg(feature = "alloc")]
pub(crate) fn retrocausal_queue_get(
    key: &StorageKey,
    peek: bool,
) -> Result<alloc::boxed::Box<[u8]>> {
    let len = retrocausal_queue_peek_len(key)?;

    let mut data = alloc::boxed::Box::<[u8]>::new_uninit_slice(len as usize);
    let mut written_len = MaybeUninit::uninit();
    sys::uninit_RetrocausalQueueConsume(
        key,
        peek,
        &mut data,
        &mut written_len,
    )?;

    debug_assert!(
        unsafe { written_len.assume_init() } == len,
        "Inconsistent RCQ size",
    );

    Ok(unsafe { data.assume_init() })
}

/// Get the next queue entry into a boxed slice.
///
#[cfg(feature = "alloc")]
pub(crate) fn retrocausal_queue_try_get(
    key: &StorageKey,
    peek: bool,
) -> Result<Option<alloc::boxed::Box<[u8]>>> {
    match retrocausal_queue_get(key, peek) {
        Ok(v) => Ok(Some(v)),
        Err(e)
            if e.is_code(
                SyscallErrorCode::kSyscallErrorCodeNoRetrocausalQueueEntry,
            ) =>
        {
            Ok(None)
        }
        Err(e) => Err(e),
    }
}
