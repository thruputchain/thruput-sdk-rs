use crate::exn::*;
use crate::sys::{self, SyscallNo};
use crate::sys_result::bytestring as result;
use crate::types::{RuntimeMemoryLength, StorageKey};
use core::mem::MaybeUninit;
use groundhog_xdr::storage_item::StorageDeltaFlags;

/// Returns the length of the bytestring stored at the given key.
///
/// Throws a syscall exception if nothing is there, or if a value of another
/// type is there.
///
pub fn bytestring_get_len(key: &StorageKey) -> u32 {
    let mut out: RuntimeMemoryLength = 0;

    sys::raw_BytestringGetLen(key, &mut out).unwrap_or_else(|errno| {
        syscall_throw(SyscallNo::kBytestringGetLen, errno);
    });

    out
}

/// Reports whether a bytestring exists at the given key.
///
/// Returns `false` if nothing is there.
///
/// If an item of another type exists there, throws a syscall exception with
/// code [`sys::SyscallErrorCode::kSyscallErrorCodeInvalidSyscallInput`].
///
pub fn bytestring_exists(key: &StorageKey) -> bool {
    result::bytestring_exists(key).unwrap_or_else(|errno| {
        syscall_throw(SyscallNo::kBytestringGetLen, errno);
    })
}

/// Sets the value at the given key to the given bytestring.
///
/// This succeeds even if a value of another type was at the location.
///
/// If concurrent contract invocations set the location to the same bytestring,
/// this will not cause any such invocations to abort.
///
pub fn bytestring_set(key: &StorageKey, slice: &[u8]) {
    sys::raw_BytestringSet(key, slice, StorageDeltaFlags::None.0)
        .unwrap_or_else(|errno| {
            syscall_throw(SyscallNo::kBytestringSet, errno);
        });
}

pub fn bytestring_set_exclusive(key: &StorageKey, slice: &[u8]) {
    result::bytestring_set_exclusive(key, slice).unwrap_or_else(|errno| {
        syscall_throw(SyscallNo::kBytestringSet, errno);
    });
}

/// Returns the bytestring contents in a statically-sized array.
///
/// Throws `LibraryExn::BytestringWrongSize` if the bytestring has a different
/// length from the one requested.  Throws other exceptions in the same
/// situations as [`bytestring_get`].
///
/// (Beware that large bytestrings may overflow your execution stack.)
///
pub fn bytestring_get_sized<const N: usize>(key: &StorageKey) -> [u8; N] {
    let mut bytes_mu = [MaybeUninit::<u8>::uninit(); N];
    let mut len = MaybeUninit::uninit();
    sys::uninit_BytestringGet(key, &mut bytes_mu, &mut len).unwrap_or_else(
        |errno| syscall_throw(SyscallNo::kBytestringGet, errno),
    );

    // Safety: The syscall returned with success, so the out-args are
    // initialized

    let len = unsafe { len.assume_init() };

    if len != N as u32 {
        library_throw(LibraryExn::BytestringWrongSize {
            expected: N as u32,
            found: len,
        });
    }

    let bytes: [u8; N] =
        unsafe { bytes_mu.map(|elt| MaybeUninit::assume_init(elt)) };

    bytes
}

#[cfg(feature = "alloc")]
mod bytestring_get_alloc {
    use super::*;
    use alloc::boxed::Box;

    pub fn bytestring_get(key: &StorageKey) -> Box<[u8]> {
        result::bytestring_get(key).unwrap_or_else(|errno| {
            syscall_throw(SyscallNo::kBytestringGet, errno);
        })
    }

    pub fn bytestring_try_get(key: &StorageKey) -> Option<Box<[u8]>> {
        result::bytestring_try_get(key).unwrap_or_else(|errno| {
            syscall_throw(SyscallNo::kBytestringGet, errno);
        })
    }
}
#[cfg(feature = "alloc")]
pub use bytestring_get_alloc::*;
