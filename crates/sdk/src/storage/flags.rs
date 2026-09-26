//! Storage flags on leaf handles.
//!
//! A handle's exclusive access mode (from `at_mut_exclusive`, `set_exclusive`
//! or `delete_exclusive`) is a write policy: each successful write through it
//! is followed by a request for `ExclusiveLock`. Building, reading and
//! dropping a handle leave the host's flags alone, and writes through an
//! ordinary handle never remove them, so a lock taken once lasts for the rest
//! of the transaction unless [`clear_flags`](Flaggable::clear_flags) removes
//! it. See [`sys_result::flags`](crate::sys_result::flags).

use super::sealed::Sealed;
use crate::StorageFlags;
use crate::types::StorageKey;

/// A leaf handle whose key's storage flags can be read and changed.
///
/// Implemented by `Value`, `NNInt`, `Balance` and `Bytestring`. Sealed, like
/// [`StorageType`](super::StorageType).
#[allow(private_bounds)]
pub trait Flaggable: Sealed {
    /// The storage key the flags belong to.
    #[doc(hidden)]
    fn storage_key(&self) -> &StorageKey;

    /// The storage flags the current transaction requests for this key, as
    /// seen by the current call frame:
    /// [`ExclusiveLock`](StorageFlags::ExclusiveLock) once an exclusive write
    /// or [`set_flags`](Self::set_flags) has asked for it, until
    /// [`clear_flags`](Self::clear_flags). A key with no pending write has
    /// none.
    fn flags(&self) -> StorageFlags {
        crate::sys_exn::flags::storage_flags(self.storage_key())
    }

    /// Adds `flags` (a bitwise OR) to the key's storage flags. Later writes
    /// through any handle keep them.
    ///
    /// Throws unless a write to the key is pending in the current call frame:
    /// one made here, or inherited from a caller or a successful callee. A
    /// value that only exists from an earlier block does not count.
    fn set_flags(&mut self, flags: StorageFlags) {
        crate::sys_exn::flags::set_storage_flags(self.storage_key(), flags)
    }

    /// Removes every storage flag from the key. Does nothing if no write to
    /// it is pending.
    fn clear_flags(&mut self) {
        crate::sys_exn::flags::clear_storage_flags(self.storage_key())
    }
}
