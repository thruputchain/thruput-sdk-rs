//! Routines for manipulating the storage type `Bytestring`.
//!
//! These routines are thin wrappers around VM syscalls.
//!
//! They return [`Result<T, E>`](core::result::Result) where `E` is a
//! [`SysErr`](crate::sys::SysErr) containing an error code in the case of a
//! syscall failure.

use crate::internal::ScratchBuf;
use crate::sys;
use crate::sys::SysResult as Result;
use crate::types::{RuntimeMemoryLength, StorageKey};
use core::mem::MaybeUninit;
use groundhog_xdr::storage_item::StorageDeltaFlags;

use super::flags::request_after_write;

/// Reports whether a bytestring exists at the given key.
///
/// Returns `false` if nothing is there.
/// If an item of another type exists there, returns an error with code
/// [`sys::SyscallErrorCode::kSyscallErrorCodeInvalidSyscallInput`].
///
pub fn bytestring_exists(key: &StorageKey) -> Result<bool> {
    match bytestring_get_len(key) {
        Ok(_) => Ok(true),
        Err(e) => {
            if e.is_storage_key_not_exist() {
                Ok(false)
            } else {
                Err(e)
            }
        }
    }
}

/// Returns the length of the bytestring stored at the given key.
///
/// Returns an error in the same situations as [`bytestring_get`].
///
pub fn bytestring_get_len(key: &StorageKey) -> Result<u32> {
    let mut out: RuntimeMemoryLength = 0;
    sys::raw_BytestringGetLen(key, &mut out)?;

    Ok(out as u32)
}

/// Sets the value at the given key to the given bytestring.
///
/// This succeeds even if a value of another type was at the location.
///
/// If concurrent contract invocations set the location to the same bytestring,
/// this will not cause any such invocations to abort.
///
/// This leaves the key's storage flags as they are: a lock requested by an
/// earlier exclusive write stays (see [`flags`](super::flags)).
///
pub fn bytestring_set(key: &StorageKey, slice: &[u8]) -> Result<()> {
    sys::raw_BytestringSet(key, slice)
}

/// Sets the value at the given key to the given bytestring.
///
/// This succeeds even if a value of another type was at the location.
///
/// If concurrent contract invocations set the bytestring exclusively, only one
/// such invocation can succeed without reverting.
///
/// The write is followed by a request for
/// [`ExclusiveLock`](StorageDeltaFlags::ExclusiveLock), which later writes to
/// the key keep (see [`flags`](super::flags)). If the write fails, no lock is
/// requested.
///
pub fn bytestring_set_exclusive(key: &StorageKey, slice: &[u8]) -> Result<()> {
    sys::raw_BytestringSet(key, slice)?;
    request_after_write(key, StorageDeltaFlags::ExclusiveLock);
    Ok(())
}

/// Returns the bytestring contents in a statically-sized array.
///
/// Returns `None` if the bytestring has the wrong length.
/// Returns an error in the same situations as [`bytestring_get`].
///
/// (Beware that large bytestrings may overflow your execution stack.)
///
pub fn bytestring_get_sized<const N: usize>(
    key: &StorageKey,
) -> Result<Option<[u8; N]>> {
    let mut bytes_mu = [MaybeUninit::<u8>::uninit(); N];
    let mut len = MaybeUninit::uninit();
    sys::uninit_BytestringGet(key, &mut bytes_mu, &mut len)?;

    // Safety: The syscall returned with success, so the out-args are
    // initialized.

    let len = unsafe { len.assume_init() };
    if len != N as u32 {
        return Ok(None);
    }

    // Apparently rustc optimizes this well enough:
    // https://github.com/rust-lang/rust/issues/96097#issuecomment-1823511470
    let bytes: [u8; N] =
        unsafe { bytes_mu.map(|elt| MaybeUninit::assume_init(elt)) };
    Ok(Some(bytes))
}

/// Read the bytestring at `key` into `buf` (cleared first). Returns
/// `Ok(None)` if the value is larger than `buf`'s capacity (only possible in
/// the no-alloc guest; values written via the typed API always fit). Filling
/// a caller-provided buffer lets the typed read paths use the reused in-place
/// scratch — no buffer of their own.
pub(crate) fn bytestring_read_into(
    key: &StorageKey,
    buf: &mut ScratchBuf,
) -> Result<Option<()>> {
    // Check length first to avoid unsafe initialization of a too-small buffer.
    let len = bytestring_get_len(key)? as usize;
    buf.clear();
    #[cfg(feature = "alloc")]
    buf.reserve(len);
    if len > buf.spare_capacity_mut().len() {
        return Ok(None);
    }
    let mut written_len = MaybeUninit::uninit();
    sys::uninit_BytestringGet(
        key,
        buf.spare_capacity_mut(),
        &mut written_len,
    )?;

    // Safety: The syscall returned with success, and we validated len <=
    // capacity, so the first `len` bytes of `buf` are initialized.
    debug_assert!(unsafe { written_len.assume_init() } as usize == len);
    unsafe { buf.set_len(len) };
    Ok(Some(()))
}

#[cfg(feature = "alloc")]
mod bytestring_get_alloc {
    use super::*;
    use alloc::boxed::Box;

    /// Returns a new allocation with the contents of the bytestring stored at
    /// the given key.
    ///
    /// Returns [`sys::SyscallErrorCode::kSyscallErrorCodeStorageKeyNotExist`]
    /// if there is none, or
    /// [`sys::SyscallErrorCode::kSyscallErrorCodeInvalidSyscallInput`] if an
    /// item of another type is there.
    ///
    /// This routine requires feature `alloc`.
    ///
    pub fn bytestring_get(key: &StorageKey) -> Result<Box<[u8]>> {
        let len = bytestring_get_len(key)?;

        let mut array = Box::<[u8]>::new_uninit_slice(len as usize);
        let mut written_len = MaybeUninit::uninit();
        sys::uninit_BytestringGet(key, &mut array, &mut written_len)?;

        debug_assert!(
            unsafe { written_len.assume_init() } == len,
            "Inconsistent bytestring size",
        );

        Ok(unsafe { array.assume_init() })
    }

    /// Like `bytestring_get`, but returns `None` (instead of an error) when no value is stored at
    /// the location.
    pub fn bytestring_try_get(key: &StorageKey) -> Result<Option<Box<[u8]>>> {
        match bytestring_get(key) {
            Ok(v) => Ok(Some(v)),
            Err(e) if e.is_storage_key_not_exist() => Ok(None),
            Err(e) => Err(e),
        }
    }
}
#[cfg(feature = "alloc")]
pub use bytestring_get_alloc::*;
