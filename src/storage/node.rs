use super::keypath::Keypath;
use super::sealed::Sealed;
use core::ops::{Deref, DerefMut};
use groundhog_xdr::storage_item::StorageDeltaFlags;

/// Marker trait for types that can appear as elements in Map and Vector.
/// Sealed — only SDK storage types implement this.
#[allow(private_bounds)]
pub trait StorageType: Sealed + From<Keypath> {}

/// A keypath-storing wrapper for compound storage field groups.
///
/// Pairs a `T: From<Keypath>` with its root keypath.
/// Derefs to T for field access.
pub struct Node<T> {
    keypath: Keypath,
    fields: T,
}

impl<T: From<Keypath>> From<Keypath> for Node<T> {
    fn from(keypath: Keypath) -> Self {
        let fields = T::from(keypath.clone());
        Self { keypath, fields }
    }
}

impl<T> Node<T> {
    pub fn keypath(&self) -> &Keypath {
        &self.keypath
    }
}

impl<T> Deref for Node<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.fields
    }
}

impl<T> DerefMut for Node<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.fields
    }
}

impl<T> Sealed for Node<T> {}
impl<T: From<Keypath>> StorageType for Node<T> {}

impl<T: super::deletable::Deletable> super::deletable::Deletable for Node<T> {
    fn delete_internal(&mut self, flags: StorageDeltaFlags) {
        self.fields.delete_internal(flags);
    }
}
