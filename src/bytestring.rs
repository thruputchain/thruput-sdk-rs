//! Routines for manipulating the storage type `Bytestring`.
//!
//! These routines are thin wrappers around VM syscalls,
//! and return the type [`SysResult<T>`](crate::sysresult::SysResult).  This is
//! is a [`Result<T, E>`](core::result::Result) where `E` is
//! a [`SysErr`](crate::SysErr) containing
//! an error code in the case of a syscall failure.

#[allow(unused_imports)] // import is for docs
use crate::SyscallErrorCode;
use crate::sys;
use crate::sysresult::SysResult as Result;
use crate::types::{RuntimeMemoryLength, StorageKey};
use crate::exn::*;
use core::mem::MaybeUninit;

#[cfg(feature = "alloc")]
pub use bytestring_get_alloc::*;
use groundhog_xdr::storage_delta_flags::StorageItemRPCFlags;

/// Reports whether a bytestring exists at the given key.
///
/// Returns `false` if nothing is there.
/// If an item of another type exists there, returns an error with code
/// [`SyscallErrorCode::kSyscallErrorCodeInvalidSyscallInput`].
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

pub fn _bytestring_get_len(key: &StorageKey) -> u32 {
    let mut out: RuntimeMemoryLength = 0;

    sys::raw_BytestringGetLen(key, &mut out).unwrap_or_else(|errno| {
        let info = SyscallExn::new(
            crate::sys::SyscallNo::kBytestringGetLen,
            errno,
        );
        crate::abort(&info);
    });

    out
}

/// Sets the value at the given key to the given bytestring.
///
/// This succeeds even if a value of another type was at the location.
///
/// If concurrent contract invocations set the location to the same bytestring,
/// this will not cause any such invocations to abort.
///
pub fn bytestring_set(key: &StorageKey, slice: &[u8]) -> Result<()> {
    sys::raw_BytestringSet(
        key,
        slice,
        StorageItemRPCFlags::kStorageNone as i32,
    )
}

/// Sets the value at the given key to the given bytestring.
///
/// This succeeds even if a value of another type was at the location.
///
/// If concurrent contract invocations set the bytestring exclusively, only one
/// such invocation can succeed.
///
/// TODO(geoff): What about mixed exclusive/non-exclusive concurrent set?
///
pub fn bytestring_set_exclusive(key: &StorageKey, slice: &[u8]) -> Result<()> {
    sys::raw_BytestringSet(
        key,
        slice,
        StorageItemRPCFlags::kStorageExclusiveLock as i32,
    )
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

    // SAFETY: The syscall returned with success, so the out-args are initialized
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
    #[allow(unused_imports)] // import is for docs
    use crate::SyscallErrorCode;
    use crate::sys;
    use crate::sysresult::SysResult as Result;
    use crate::types::StorageKey;
    use alloc::boxed::Box;
    use core::mem::MaybeUninit;

    /// Returns a new allocation with the contents of the bytestring stored at the given key.
    ///
    /// Returns [`SyscallErrorCode::kSyscallErrorCodeStorageKeyNotExist`] if there is none,
    /// or [`SyscallErrorCode::kSyscallErrorCodeInvalidSyscallInput`] if an item of another type is there.
    ///
    /// This routine requires feature `alloc`.
    ///
    pub fn bytestring_get(key: &StorageKey) -> Result<Box<[u8]>> {
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

    /// Like `bytestring_get`, but returns `None` (instead of an error) when no value is stored at
    /// the location.
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
        let mut written_len = MaybeUninit::uninit();
        sys::uninit_BytestringGet(key, &mut vec, &mut written_len)?;

        let written_len = unsafe { written_len.assume_init() };
        if written_len != len {
            panic!("Inconsistent bytestring size");
        }

        Ok(Some(unsafe { vec.assume_init() }))
    }
}
