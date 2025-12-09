//! A handle is a typed wrapper around a `Keypath`.  It is used as a
//! typed reference to a storage location.

use super::error::Result;
use super::keypath::*;
use super::load::*;
use super::store::*;
use core::marker::PhantomData;

/// A handle to a storage location
#[derive(Debug)]
pub struct Handle<T> {
    key: KeyCache,

    // `Handle` is covariant in `T`
    _marker: PhantomData<fn() -> T>,
}

impl<T> From<Keypath> for Handle<T> {
    fn from(keypath: Keypath) -> Self {
        Self {
            key: KeyCache::from(keypath),

            _marker: PhantomData,
        }
    }
}

impl_load_via_keypath! { Handle<T> }
impl_default_via_keypath! { Handle<T> }
impl_read_via_keypath! { Handle<T> }
impl_write_xdr_empty! { Handle<T> }

// TODO: Make better use of the keycache.  Currently we don't take the
// opportunity to cache a StorageKey in the get operations (partly because a
// storage key is not necessarily computed in `load_keypath`). This may require
// adding a `load_keycache` method to the `LoadKeypath` trait.

impl<T> Handle<T> {
    pub fn new(keypath: &Keypath) -> Self {
        Self {
            key: KeyCache::new(keypath),

            _marker: PhantomData,
        }
    }

    pub fn root() -> Self {
        let root_keypath = Keypath::new();

        Self::from(root_keypath)
    }

    pub fn keypath(&self) -> &Keypath {
        self.key.keypath()
    }

    #[allow(dead_code)]
    pub(crate) fn get(&self) -> Result<T>
    where
        T: LoadKeypath,
    {
        LoadKeypath::load_keypath(self.keypath())
    }

    pub(crate) fn try_get(&self) -> Result<Option<T>>
    where
        T: LoadKeypath,
    {
        LoadKeypath::try_load_keypath(self.keypath())
    }

    pub(crate) fn get_or_default(&self) -> Result<T>
    where
        T: LoadKeypath + DefaultFromKeypath,
    {
        Ok(self.try_get()?.unwrap_or_else(|| {
            DefaultFromKeypath::default_from_keypath(self.keypath())
        }))
    }

    pub(crate) fn set(&mut self, val: &T) -> Result<()>
    where
        T: Store,
    {
        val.store(&self.key)
    }

    pub fn load(&self) -> Result<HandleRef<'_, T>>
    where
        T: LoadKeypath + DefaultFromKeypath,
    {
        let val = self.get_or_default()?;
        Ok(HandleRef::new(self, val))
    }

    pub fn load_mut(&mut self) -> Result<HandleRefMut<'_, T>>
    where
        T: LoadKeypath + DefaultFromKeypath + Store,
    {
        let val = self.get_or_default()?;
        Ok(HandleRefMut::new(self, val))
    }
}

pub use refs::*;
mod refs {
    use super::Handle;
    use crate::storage::Store;
    use core::marker::PhantomData;
    use core::ops::{Deref, DerefMut};

    /// An immutable reference to a storage location
    #[derive(Clone)]
    pub struct HandleRef<'a, T> {
        handle: PhantomData<&'a Handle<T>>,
        val: T,
    }

    impl<'a, T> HandleRef<'a, T> {
        pub fn new(_handle: &'a Handle<T>, val: T) -> Self {
            Self {
                handle: PhantomData,
                val,
            }
        }
    }

    impl<T> Deref for HandleRef<'_, T> {
        type Target = T;

        fn deref(&self) -> &Self::Target {
            &self.val
        }
    }

    /// A mutable reference to a storage location
    pub struct HandleRefMut<'a, T: Store> {
        handle: &'a mut Handle<T>,
        val: T,
    }

    // TODO: Add a dirty flag to avoid unnecessary stores.
    // TODO: Store the value as `Option<T>` and add a `delete` operation.

    impl<'a, T: Store> HandleRefMut<'a, T> {
        pub fn new(handle: &'a mut Handle<T>, val: T) -> Self {
            Self { handle, val }
        }
    }

    impl<T: Store> Deref for HandleRefMut<'_, T> {
        type Target = T;

        fn deref(&self) -> &Self::Target {
            &self.val
        }
    }

    impl<T: Store> DerefMut for HandleRefMut<'_, T> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.val
        }
    }

    impl<T: Store> Drop for HandleRefMut<'_, T> {
        fn drop(&mut self) {
            // Store the current value back to the key-value store
            self.handle.set(&self.val).unwrap();
        }
    }
}
