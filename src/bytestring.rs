//! Routines for manipulating the storage type `Bytestring`.
//!
//! These routines are thin wrappers around VM syscalls,
//! and return the type [`SysResult<T>`](crate::sysresult::SysResult).  This is
//! is a [`Result<T, E>`](core::result::Result) where `E` is
//! a [`SysErr`](crate::SysErr) containing
//! an error code in the case of a syscall failure.

use crate::sys;
use crate::sysresult::SysResult as Result;
use crate::types::{RuntimeMemoryLength, StorageKeyContractData};
#[allow(unused_imports)] // import is for docs
use crate::SyscallErrorCode;
use core::mem::MaybeUninit;

#[cfg(feature = "alloc")]
pub use bytestring_get_alloc::*;

/// Reports whether a bytestring exists at the given key.
///
/// Returns `false` if nothing is there.
/// If an item of another type exists there, returns an error with code
/// [`SyscallErrorCode::kSyscallErrorCodeInvalidSyscallInput`].
///
pub fn bytestring_exists(key: &StorageKeyContractData) -> Result<bool> {
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
pub fn bytestring_get_len(key: &StorageKeyContractData) -> Result<u32> {
    let mut out: RuntimeMemoryLength = 0;
    sys::raw_BytestringGetLen(key, &mut out)?;

    Ok(out as u32)
}

/// Sets the contents of the bytestring at the given key.
/// (**TODO**: Check semantics)
///
/// Creates the bytestring item if the location is empty.
/// Returns [`SyscallErrorCode::kSyscallErrorCodeInvalidSyscallInput`] if an item of another type is there.
/// If another contract instance running in parallel sets the bytestring
/// to the same value, both writes will be allowed.
///
pub fn bytestring_set(
    key: &StorageKeyContractData,
    slice: &[u8],
) -> Result<()> {
    sys::raw_BytestringSet(key, slice /*, 0 */)?;
    Ok(())
}

/*
/// Sets the contents of the bytestring at the given key.
/// (**TODO**: Check semantics)
///
/// Creates the bytestring item if the location is empty.
/// Returns [`SyscallErrorCode::kSyscallErrorCodeInvalidSyscallInput`] if an item of another type is there.
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
*/

/// Returns the bytestring contents in a statically-sized array.
///
/// Returns `None` if the bytestring has the wrong length.
/// Returns an error in the same situations as [`bytestring_get`].
/// Beware that large bytestrings may overflow your execution stack.
///
pub fn bytestring_get_sized<const N: usize>(
    key: &StorageKeyContractData,
) -> Result<Option<[u8; N]>> {
    let mut bytes_mu = [MaybeUninit::<u8>::uninit(); N];
    let mut len = MaybeUninit::uninit();
    sys::uninit_BytestringGet(key, &mut bytes_mu, &mut len)?;

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

#[cfg(feature = "alloc")]
mod bytestring_get_alloc {
    use crate::sys;
    use crate::sysresult::SysResult as Result;
    use crate::types::StorageKeyContractData;
    #[allow(unused_imports)] // import is for docs
    use crate::SyscallErrorCode;
    use alloc::boxed::Box;
    use core::mem::MaybeUninit;

    /// Returns a new allocation with the contents of the bytestring stored at the given key.
    ///
    /// Returns [`SyscallErrorCode::kSyscallErrorCodeStorageKeyNotExist`] if there is none,
    /// or [`SyscallErrorCode::kSyscallErrorCodeInvalidSyscallInput`] if an item of another type is there.
    /// This routine requires feature `alloc`.
    ///
    pub fn bytestring_get(key: &StorageKeyContractData) -> Result<Box<[u8]>> {
        let len = super::bytestring_get_len(key)?;
        let mut vec = Box::<[u8]>::new_uninit_slice(len as usize);
        let mut written_len = MaybeUninit::uninit();
        sys::uninit_BytestringGet(key, &mut vec, &mut written_len)?;

        let written_len = unsafe { written_len.assume_init() };
        if written_len != len {
            panic!("Inconsistent bytestring size");
        }

        Ok(unsafe { vec.assume_init() })
    }

    pub fn bytestring_try_get(
        key: &StorageKeyContractData,
    ) -> Result<Option<Box<[u8]>>> {
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
        let mut written_len = MaybeUninit::uninit();
        sys::uninit_BytestringGet(key, &mut vec, &mut written_len)?;

        let written_len = unsafe { written_len.assume_init() };
        if written_len != len {
            panic!("Inconsistent bytestring size");
        }

        Ok(Some(unsafe { vec.assume_init() }))
    }
}
