//! System functions: retro-causal queues

use crate::sys;
use crate::sys::SysResult as Result;
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

fn retrocausal_queue_get_sized<const N: usize>(
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

/// Get the next queue entry.
///
/// A vector of maximum length `N` is returned.
///
#[cfg(feature = "alloc")]
pub fn retrocausal_queue_get_max<const N: usize>(
    key: &StorageKey,
    peek: bool,
) -> Result<crate::Vec<u8>>
{
    let mut v = crate::Vec::with_capacity(N);

    let data = v.spare_capacity_mut();
    let mut written_len: MaybeUninit<RuntimeMemoryLength> =
        MaybeUninit::uninit();

    sys::uninit_RetrocausalQueueConsume(
        key,
        peek,
        data,
        &mut written_len,
    )?;

    // Safety: Successful syscall return means the indicated bytes have been
    // filled.
    unsafe { v.set_len(written_len.assume_init() as usize); }

    Ok(v)
}
