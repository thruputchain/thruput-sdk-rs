//! System functions: retro-causal queues

use crate::internal::ScratchBuf;
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

// Every queue entry fits the scratch buffer, so the read paths below need no
// peek-len pre-flight to size it.
const _: () = assert!(
    thruput_xdr::retrocausal_queue::kQueueEntryMaxLen as usize
        <= crate::internal::SCRATCH_MAX
);

/// Read the next queue entry into `buf` (cleared first), in a single
/// consume/peek syscall.
///
/// Returns `Ok(None)` if the entry exceeds `buf`'s capacity — unreachable
/// when `buf` is the SDK scratch (see the assert above), and the host
/// truncates rather than fails, so with `peek == false` an `Ok(None)` entry
/// has still been consumed. `try_` paths that must not destroy the entry on
/// failure read with `peek == true` and pop separately (see
/// [`retrocausal_queue_consume_discard`]). Filling a caller-provided buffer
/// lets the typed read paths use the reused in-place scratch.
pub(crate) fn retrocausal_queue_read_into(
    key: &StorageKey,
    peek: bool,
    buf: &mut ScratchBuf,
) -> Result<Option<()>> {
    buf.clear();
    #[cfg(feature = "alloc")]
    buf.reserve(thruput_xdr::retrocausal_queue::kQueueEntryMaxLen as usize);
    let mut written_len = MaybeUninit::uninit();
    sys::uninit_RetrocausalQueueConsume(
        key,
        peek,
        buf.spare_capacity_mut(),
        &mut written_len,
    )?;
    // The host reports the entry's full length even when the copy was
    // truncated to fit the buffer.
    let len = unsafe { written_len.assume_init() } as usize;
    if len > buf.spare_capacity_mut().len() {
        return Ok(None);
    }
    unsafe { buf.set_len(len) };
    Ok(Some(()))
}

/// Pop the next queue entry, discarding its contents: the zero-length output
/// span makes the host copy nothing, so only the removal takes effect.
pub(crate) fn retrocausal_queue_consume_discard(
    key: &StorageKey,
) -> Result<()> {
    // A real stack address with length zero, rather than an empty slice's
    // dangling pointer, so every dispatch path sees an in-bounds span.
    let mut sink = [MaybeUninit::<u8>::uninit(); 1];
    let mut written_len = MaybeUninit::uninit();
    sys::uninit_RetrocausalQueueConsume(
        key,
        false,
        &mut sink[..0],
        &mut written_len,
    )
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
