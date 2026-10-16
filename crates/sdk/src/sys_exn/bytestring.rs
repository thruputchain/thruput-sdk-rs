use crate::exn::*;
use crate::re_throw::OrSyscallThrow;
use crate::sys::{self, SyscallNo};
use crate::sys_result::bytestring as result;
use crate::types::{RuntimeMemoryLength, StorageKey};
use core::mem::MaybeUninit;

/// Returns the length of the bytestring stored at the given key.
///
/// Throws a syscall exception if nothing is there, or if a value of another
/// type is there.
///
pub fn bytestring_get_len(key: &StorageKey) -> u32 {
    let mut out: RuntimeMemoryLength = 0;

    sys::raw_BytestringGetLen(key, &mut out)
        .or_syscall_throw(SyscallNo::kBytestringGetLen);

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
    result::bytestring_exists(key)
        .or_syscall_throw(SyscallNo::kBytestringGetLen)
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
pub fn bytestring_set(key: &StorageKey, slice: &[u8]) {
    sys::raw_BytestringSet(key, slice)
        .or_syscall_throw(SyscallNo::kBytestringSet);
}

/// Sets the value at the given key to the given bytestring, then requests
/// [`ExclusiveLock`](crate::StorageDeltaFlags::ExclusiveLock) for the key.
///
/// If concurrent contract invocations set the bytestring exclusively, only one
/// such invocation can succeed without reverting. Later writes to the key in
/// this transaction keep the lock.
///
pub fn bytestring_set_exclusive(key: &StorageKey, slice: &[u8]) {
    result::bytestring_set_exclusive(key, slice)
        .or_syscall_throw(SyscallNo::kBytestringSet);
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
    sys::uninit_BytestringGet(key, &mut bytes_mu, &mut len)
        .or_syscall_throw(SyscallNo::kBytestringGet);

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
        use crate::re_throw::OrReThrow;
        bytestring_try_get(key).or_re_throw()
    }

    /// Read the bytestring at `key`, reporting failure as `Err` instead of
    /// aborting (the `try_` law; see the [`exn`](mod@crate::exn) module
    /// docs). A missing key is a `LibraryExn::Syscall` for which
    /// [`is_storage_missing`](LibraryExn::is_storage_missing) is true; a
    /// value of another storage type or any other syscall failure is a
    /// `LibraryExn::Syscall` as well.
    pub fn bytestring_try_get(
        key: &StorageKey,
    ) -> Result<Box<[u8]>, LibraryExn> {
        result::bytestring_get(key).or_syscall_exn(SyscallNo::kBytestringGet)
    }
}
#[cfg(feature = "alloc")]
pub use bytestring_get_alloc::*;
