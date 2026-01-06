use super::error::Result;
use super::keypath::Keypath;
use super::load::LoadKeypath;
use super::store::*;
use crate::types::StorageKey;
use crate::{delete_exclusive, delete_shared};
use core::fmt::Debug;
use core::marker::PhantomData;
use xdrs::WriteXDR;

#[derive(Debug)]
pub struct Map<K, V> {
    keypath: Keypath,

    // The type should be covariant in K and V, but (for drop
    // purposes) doesn't actually own values of either type
    _marker: PhantomData<fn() -> (K, V)>,
}

impl<K, V> From<Keypath> for Map<K, V> {
    fn from(keypath: Keypath) -> Self {
        let _marker = PhantomData;
        Self { keypath, _marker }
    }
}

impl_load_via_keypath! { Map<K, V> }
impl_default_via_keypath! { Map<K, V> }
impl_read_via_keypath! { Map<K, V> }
impl_write_xdr_empty! { Map<K, V> }

impl<K, V> Map<K, V> {
    pub fn new(keypath: &Keypath) -> Self {
        Self {
            keypath: keypath.clone(),
            _marker: PhantomData,
        }
    }

    pub fn keypath(&self) -> &Keypath {
        &self.keypath
    }

    pub fn get(&self, entry_key: &K) -> Result<V>
    where
        K: WriteXDR,
        V: LoadKeypath,
    {
        let keypath = self.make_entry_keypath(entry_key);

        LoadKeypath::load_keypath(&keypath)
    }

    pub fn try_get(&self, entry_key: &K) -> Result<Option<V>>
    where
        K: WriteXDR,
        V: LoadKeypath,
    {
        let keypath = self.make_entry_keypath(entry_key);

        LoadKeypath::try_load_keypath(&keypath)
    }

    pub fn set(&mut self, entry_key: &K, value: &V) -> Result<()>
    where
        K: WriteXDR,
        V: Store,
    {
        let key = self.make_entry_storage_key(entry_key);

        value.store(&key)
    }

    pub fn set_exclusive(&mut self, entry_key: &K, value: &V) -> Result<()>
    where
        K: WriteXDR,
        V: Store,
    {
        let key = self.make_entry_storage_key(entry_key);

        value.store_exclusive(&key)
    }

    pub fn delete(&mut self, entry_key: &K) -> Result<()>
    where
        K: WriteXDR,
    {
        let key = self.make_entry_storage_key(entry_key);

        delete_shared(&key)?;

        Ok(())
    }

    pub fn delete_exclusive(&mut self, entry_key: &K) -> Result<()>
    where
        K: WriteXDR,
    {
        let key = self.make_entry_storage_key(entry_key);

        delete_exclusive(&key)?;

        Ok(())
    }

    pub fn entry<'m, 'k>(
        &'m mut self,
        entry_key: &'k K,
    ) -> Result<Entry<'m, 'k, K, V>>
    where
        K: WriteXDR,
        V: LoadKeypath + Store,
    {
        let keypath = self.make_entry_keypath(entry_key);
        let value = LoadKeypath::try_load_keypath(&keypath)?;

        Ok(Entry::new(self, entry_key, value, keypath))
    }

    fn make_entry_keypath(&self, entry_key: &K) -> Keypath
    where
        K: WriteXDR,
    {
        let mut keypath = self.keypath.clone();
        keypath.push_item(entry_key);
        keypath
    }

    fn make_entry_storage_key(&self, entry_key: &K) -> StorageKey
    where
        K: WriteXDR,
    {
        let keypath = self.make_entry_keypath(entry_key);
        keypath.generate_key()
    }
}

/// A view into a single entry in the map
pub struct Entry<'m, 'k, K, V>
where
    K: WriteXDR,
    V: Store,
{
    // A change to the implementation may require a reference to the map,
    // so we keep this here to enforce the lifetime constraints:
    map: PhantomData<&'m mut Map<K, V>>,

    entry_key: &'k K,
    value: Option<V>,
    keypath: Keypath,
    dirty: bool,
}

// Note: If we omit the `entry_key` field (and give up the `key` method),
// `Entry` could be replaced with `HandleRefMut`.  Or we could add a
// separate method `Map::entry_handle(..) -> HandleRefMut<..>`.

impl<'m, 'k, K, V> Entry<'m, 'k, K, V>
where
    K: WriteXDR,
    V: Store,
{
    pub fn new(
        _map: &'m Map<K, V>,
        entry_key: &'k K,
        value: Option<V>,
        keypath: Keypath,
    ) -> Self {
        Self {
            map: PhantomData,
            entry_key,
            value,
            keypath,
            dirty: false,
        }
    }

    /// Get a reference to the entry's key.
    pub fn key(&self) -> &'k K {
        self.entry_key
    }

    /// Get a reference to the entry value.
    pub fn get(&self) -> &Option<V> {
        &self.value
    }

    /// Get a mutable reference to the entry value.  Changes will
    /// be flushed to storage when this `Entry` is dropped.
    pub fn get_mut(&mut self) -> &mut Option<V> {
        self.dirty = true;

        &mut self.value
    }

    /// Set the entry to a new value.
    pub fn set(&mut self, value: &V) -> Result<()>
    where
        V: Clone,
    {
        let new_value = value.clone();

        self._set(Some(new_value))
    }

    /// Delete the entry.
    pub fn delete(&mut self) -> Result<()> {
        self._set(None)
    }

    fn _set(&mut self, value: Option<V>) -> Result<()> {
        // Cache the new value
        self.value = value;
        self.dirty = true;

        // Store the now-current value
        self.flush()
    }

    /// Store the current entry value back to its storage location
    /// if it could have changed since being loaded.  (It is usually
    /// not necessary to call this because it is called automatically
    /// when the `Entry` is dropped.)
    pub fn flush(&mut self) -> Result<()> {
        if !self.dirty {
            return Ok(());
        }

        let key = self.make_storage_key();

        // Store the current value
        let res = if let Some(value) = &self.value {
            value.store(&key)
        } else {
            delete_shared(&key).map_err(|e| e.into())
        };

        if res.is_ok() {
            self.dirty = false;
        }

        res
    }

    // We could cache this (as well as the keypath), but we don't expect
    // typical usage patterns to include multiple `set` calls for an entry.
    // (We could also record the key constructed in `Map::entry()`, but there
    // might not have been one because some objects can be loaded directly from
    // a keypath.)
    fn make_storage_key(&self) -> StorageKey
    where
        K: WriteXDR,
    {
        self.keypath.generate_key()
    }
}

impl<'m, 'k, K, V> Drop for Entry<'m, 'k, K, V>
where
    K: WriteXDR,
    V: Store,
{
    fn drop(&mut self) {
        self.flush().unwrap_or_else(|e| {
            panic!("Failed flushing a Map::Entry to the store: {}", e)
        })
    }
}
