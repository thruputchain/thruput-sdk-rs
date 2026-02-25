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
    // TODO(geoff) or TODO(dobog): figure out a more natural rcq iface.
    let mut queue_entry = [MaybeUninit::uninit(); N];
    let mut written_len: MaybeUninit<RuntimeMemoryLength> =
        MaybeUninit::uninit();
    sys::uninit_RetrocausalQueueConsume(
        key,
        false,
        &mut queue_entry,
        &mut written_len,
    )?;
    // This assertion is silly but if we're going to call assume_init on all N bytes,
    // then (while technically u8 is valid for any bit pattern) the bytes beyond written_len
    // to the end are not modified by the syscall.
    let written_len = unsafe { written_len.assume_init() };
    assert_eq!(written_len, N as u32);
    let bytes: [u8; N] =
        unsafe { queue_entry.map(|elt| MaybeUninit::assume_init(elt)) };
    Ok(bytes)
}

pub fn retrocausal_queue_peek_sized<const N: usize>(
    key: &StorageKey,
) -> Result<[u8; N]> {
    let mut queue_entry = [MaybeUninit::uninit(); N];
    let mut written_len: MaybeUninit<RuntimeMemoryLength> =
        MaybeUninit::uninit();
    sys::uninit_RetrocausalQueueConsume(
        key,
        true,
        &mut queue_entry,
        &mut written_len,
    )?;
    let written_len = unsafe { written_len.assume_init() };
    assert_eq!(written_len, N as u32);
    let bytes: [u8; N] =
        unsafe { queue_entry.map(|elt| MaybeUninit::assume_init(elt)) };
    Ok(bytes)
}
