//! Routines for manipulating the storage type `Bytestring`.
//!
//! These routines are thin wrappers around VM syscalls,
//! and return the type [`SysResult<T>`](crate::result::SysResult).  This is
//! is a standard [`Result<T, E>`](core::result::Result) where `E` is
//! a [`SysErr`](crate::syscall::SysErr) that contains
//! an error code in the case of an underlying syscall failure.

use crate::result::SysResult as Result;
use crate::sys;
#[allow(unused_imports)]
use crate::syscall::SyscallErrorCode;
use crate::types::{RuntimeMemoryLength, StorageKey};
use core::mem::MaybeUninit;

#[cfg(feature = "alloc")]
pub use bytestring_get_alloc::*;

/// Reports whether a bytestring exists at the given key.
///
/// Returns `false` if nothing is there.
/// If an item of another type exists there, returns an error with code
/// [`SyscallErrorCode::kInvalidInput`].
///
pub fn bytestring_exists(key: &StorageKey) -> Result<bool> {
    match bytestring_get_len(key) {
        Ok(_) => Ok(true),
        Err(error) => {
            if error.is_storage_key_not_exist() {
                Ok(false)
            } else {
                Err(error)
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

/// Sets the contents of the bytestring at the given key.
/// (**TODO**: Check semantics)
///
/// Creates the bytestring item if the location is empty.
/// Returns [`SyscallErrorCode::kInvalidInput`] if an item of another type is there.
/// If another contract instance running in parallel sets the bytestring
/// to the same value, both writes will be allowed.
///
pub fn bytestring_set(key: &StorageKey, slice: &[u8]) -> Result<()> {
    sys::raw_BytestringSet(key, slice, 0)?;
    Ok(())
}

/// Sets the contents of the bytestring at the given key.
/// (**TODO**: Check semantics)
///
/// Creates the bytestring item if the location is empty.
/// Returns [`SyscallErrorCode::kInvalidInput`] if an item of another type is there.
/// If another contract instance running in parallel sets the bytestring
/// to the same value, the contract instance that writes last will be aborted.
///
pub fn bytestring_set_exclusive(key: &StorageKey, slice: &[u8]) -> Result<()> {
    sys::raw_BytestringSet(
        key,
        slice,
        crate::types::StorageDeltaModifier::kLockExclusive as i32,
    )
}

/// Returns the bytestring contents in a statically-sized array.
///
/// Returns `None` if the bytestring has the wrong length.
/// Returns an error in the same situations as [`bytestring_get`].
/// Beware that large bytestrings may overflow your execution stack.
///
pub fn bytestring_get_sized<const N: usize>(
    key: &StorageKey,
) -> Result<Option<[u8; N]>> {
    let mut bytes_mu = [MaybeUninit::<u8>::uninit(); N];
    let mut len = 0;
    sys::uninit_BytestringGet(key, &mut bytes_mu, &mut len)?;
    if len != N as u32 {
        return Ok(None);
    }

    let bytes: [u8; N] = unsafe { MaybeUninit::array_assume_init(bytes_mu) };
    Ok(Some(bytes))
}

#[cfg(feature = "alloc")]
mod bytestring_get_alloc {
    use crate::result::SysResult as Result;
    use crate::sys;
    #[allow(unused_imports)]
    use crate::syscall::SyscallErrorCode;
    use crate::types::StorageKey;
    use ::alloc::boxed::Box;

    /// Returns a new allocation with the contents of the bytestring stored at the given key.
    ///
    /// Returns [`SyscallErrorCode::kStorageKeyNotExist`] if there is none,
    /// or [`SyscallErrorCode::kInvalidInput`] if an item of another type is there.
    /// This routine requires feature `alloc`.
    ///
    pub fn bytestring_get(key: &StorageKey) -> Result<Box<[u8]>> {
        let len = super::bytestring_get_len(key)?;
        let mut vec = Box::<[u8]>::new_uninit_slice(len as usize);
        let mut written_len = 0;
        sys::uninit_BytestringGet(key, &mut vec, &mut written_len)?;
        if written_len != len {
            panic!("Inconsistent bytestring size");
        }

        Ok(unsafe { vec.assume_init() })
    }

    pub fn bytestring_try_get(key: &StorageKey) -> Result<Option<Box<[u8]>>> {
        let len = match super::bytestring_get_len(key) {
            Ok(len) => len,
            Err(e) => {
                if e.is_storage_key_not_exist() {
                    return Ok(None);
                } else {
                    return Err(e);
                }
            }
        };
        let mut vec = Box::<[u8]>::new_uninit_slice(len as usize);
        let mut written_len = 0;
        sys::uninit_BytestringGet(key, &mut vec, &mut written_len)?;
        if written_len != len {
            panic!("Inconsistent bytestring size");
        }

        Ok(Some(unsafe { vec.assume_init() }))
    }
}
