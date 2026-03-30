// src/storage/value.rs
use super::keypath::*;
use super::node::StorageNode;
use core::fmt::Debug;
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};

/// A typed storage slot for serialized values.
///
/// `Value<T>` is a handle to a storage location that holds a serialized `T`.
/// All operations are immediate syscalls — no buffering.
/// For in-place modification with auto-flush, use `load_mut()`.
#[derive(Debug)]
pub struct Value<T> {
    key: KeyCache,
    _marker: PhantomData<fn() -> T>,
}

impl<T> From<Keypath> for Value<T> {
    fn from(keypath: Keypath) -> Self {
        Self {
            key: KeyCache::from(keypath),
            _marker: PhantomData,
        }
    }
}

impl_load_via_keypath! { Value<T> }
impl_default_via_keypath! { Value<T> }
impl_read_via_keypath! { Value<T> }
impl_write_xdr_empty! { Value<T> }

impl<T> StorageNode for Value<T> {
    fn from_keypath(keypath: Keypath) -> Self {
        Self::from(keypath)
    }

    fn keypath(&self) -> &Keypath {
        self.key.keypath()
    }
}

impl<T> Value<T> {
    pub fn new(keypath: &Keypath) -> Self {
        Self {
            key: KeyCache::new(keypath),
            _marker: PhantomData,
        }
    }
}

impl<T> Value<T>
where
    T: super::load::LoadKeypath,
{
    /// Deserialize and return the stored value.
    /// Aborts if the key does not exist.
    pub fn get(&self) -> T {
        super::load::LoadKeypath::load_keypath(self.key.keypath())
            .unwrap_or_else(|e| crate::exn::library_throw(e))
    }

    /// Deserialize and return the stored value, or None if the key doesn't exist.
    pub fn try_get(&self) -> Option<T> {
        super::load::LoadKeypath::try_load_keypath(self.key.keypath())
            .unwrap_or_else(|e| crate::exn::library_throw(e))
    }
}

impl<T> Value<T>
where
    T: super::store::Store,
{
    /// Serialize and store the value immediately.
    pub fn set(&mut self, value: &T) {
        if self.key.keypath().is_exclusive() {
            value.store_exclusive(&self.key);
        } else {
            value.store(&self.key);
        }
    }
}

impl<T> Value<T> {
    /// Delete the value at this storage location.
    pub fn delete(&mut self) {
        if self.key.keypath().is_exclusive() {
            crate::delete_exclusive(&self.key);
        } else {
            crate::delete_shared(&self.key);
        }
    }
}

impl<T> Value<T>
where
    T: super::load::LoadKeypath + DefaultFromKeypath + super::store::Store,
{
    /// Load the value for in-place modification.
    /// Returns a `ValueMut` that derefs to `&mut T` and flushes on drop.
    pub fn load_mut(&mut self) -> ValueMut<'_, T> {
        let data = T::try_load_keypath(self.key.keypath())
            .unwrap_or_else(|e| crate::exn::library_throw(e))
            .unwrap_or_else(|| T::default_from_keypath(self.key.keypath()));
        ValueMut { value: self, data }
    }
}

/// Mutable view into a `Value<T>`. Flushes T back to storage on drop.
pub struct ValueMut<'a, T: super::store::Store> {
    value: &'a mut Value<T>,
    data: T,
}

impl<T: super::store::Store> Deref for ValueMut<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.data
    }
}

impl<T: super::store::Store> DerefMut for ValueMut<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.data
    }
}

impl<T: super::store::Store> Drop for ValueMut<'_, T> {
    fn drop(&mut self) {
        self.value.set(&self.data);
    }
}
