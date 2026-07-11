use super::keypath::{KeyCache, Keypath};
use super::node::FromConstPath;
use crate::sys_exn::bytestring::*;
use core::fmt::Debug;
use groundhog_xdr::storage_item::StorageDeltaFlags;

#[derive(Debug)]
pub struct Bytestring {
    key: KeyCache,
}

impl From<Keypath> for Bytestring {
    fn from(keypath: Keypath) -> Self {
        Self {
            key: KeyCache::from(keypath),
        }
    }
}

impl Bytestring {
    /// Returns the number of bytes stored at this key.
    ///
    /// **Aborts** the transaction (throws a syscall exception) if nothing is
    /// stored here, or if a value of another type occupies the key. Use
    /// [`exists`](Self::exists) to test for presence without aborting, or
    /// [`try_get`](Self::try_get) to read an optional value.
    pub fn len(&self) -> usize {
        bytestring_get_len(&self.key) as usize
    }

    /// Returns `true` if the stored bytestring has zero length (`len() == 0`).
    ///
    /// Shares [`len`](Self::len)'s abort behavior exactly: this **aborts** the
    /// transaction on a never-set key rather than reporting "empty". An empty
    /// bytestring and a missing key are distinct states here — use
    /// [`exists`](Self::exists) to tell them apart without aborting. (This
    /// differs deliberately from [`Vector::is_empty`](super::vector::Vector::is_empty),
    /// which treats a never-set vector as empty.)
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns `true` if a bytestring is stored at this key, `false` if nothing
    /// is.
    ///
    /// Unlike [`len`](Self::len)/[`is_empty`](Self::is_empty)/[`get`](Self::get)/[`get_sized`](Self::get_sized),
    /// this does not abort on a missing key — it is the way to distinguish a
    /// missing key from a present-but-empty bytestring. (It still throws if a
    /// value of another type occupies the key.)
    pub fn exists(&self) -> bool {
        bytestring_exists(&self.key)
    }

    /// Stores `buf` at this key, overwriting any existing value (including one
    /// of another type). Uses an exclusive store if this key's keypath is
    /// exclusive.
    pub fn set(&mut self, buf: &[u8]) {
        if self.key.is_exclusive() {
            bytestring_set_exclusive(&self.key, buf);
        } else {
            bytestring_set(&self.key, buf);
        }
    }

    /// Reads the stored bytestring into a fixed-size array.
    ///
    /// **Aborts** if the key is missing, holds a value of another type, or
    /// holds a bytestring whose length is not exactly `N`
    /// (`LibraryExn::BytestringWrongSize`). Beware that large `N` may overflow
    /// the execution stack.
    pub fn get_sized<const N: usize>(&self) -> [u8; N] {
        bytestring_get_sized(&self.key)
    }

    /// Reads the stored bytestring.
    ///
    /// **Aborts** if the key is missing or holds a value of another type. Use
    /// [`try_get`](Self::try_get) to handle those cases gracefully.
    #[cfg(feature = "alloc")]
    pub fn get(&self) -> alloc::boxed::Box<[u8]> {
        bytestring_get(&self.key)
    }

    /// Reads the stored bytestring, reporting failure as `Err` instead of
    /// aborting (the `try_` law; see the [`exn`](mod@crate::exn) module
    /// docs).
    ///
    /// A missing key is an `Err` for which
    /// [`is_storage_missing`](crate::exn::LibraryExn::is_storage_missing)
    /// is true — use
    /// [`none_if_missing`](crate::exn::NoneIfMissing::none_if_missing) to
    /// treat just that case as an ordinary `None`. A value of another
    /// storage type or any other syscall failure is a `LibraryExn::Syscall`.
    #[cfg(feature = "alloc")]
    pub fn try_get(
        &self,
    ) -> Result<alloc::boxed::Box<[u8]>, crate::exn::LibraryExn> {
        bytestring_try_get(&self.key)
    }
}

impl super::sealed::Sealed for Bytestring {}
impl super::node::StorageType for Bytestring {}

impl FromConstPath for Bytestring {
    /// Construct a `Bytestring` from a `'static` const preimage path.
    /// Uses the `Const` arm of `KeyCache` — no keypath copy.
    #[inline]
    fn __sdk_from_const_path(path: &'static [u8]) -> Self {
        Self {
            key: KeyCache::from_const(path),
        }
    }
}

impl super::deletable::Deletable for Bytestring {
    fn delete_internal(&mut self, flags: StorageDeltaFlags) {
        let keypath_flags = if self.key.is_exclusive() {
            StorageDeltaFlags::ExclusiveLock
        } else {
            StorageDeltaFlags::None
        };
        let effective = StorageDeltaFlags(flags.0 | keypath_flags.0);
        crate::sys_exn::delete::delete_with_flags(self.key.get(), effective);
    }
}
