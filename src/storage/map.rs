use super::error::Result;
use super::keypath::Keypath;
use super::load::LoadKeypath;
use super::store::*;
use crate::delete_shared;
use crate::types::StorageKey;
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

    pub fn delete(&mut self, entry_key: &K) -> Result<()>
    where
        K: WriteXDR,
    {
        let key = self.make_entry_storage_key(entry_key);

        delete_shared(&key)?;

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
        let keypath = &self.make_entry_keypath(entry_key);
        let value = LoadKeypath::try_load_keypath(keypath)?;

        Ok(Entry { entry_key, value, map: self, dirty: false })
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
    entry_key: &'k K,
    value: Option<V>,
    map: &'m mut Map<K, V>,
    dirty: bool,
    // XXX: cache the keypath/storagekey (instead of the entry_key)?
}

impl<'m, 'k, K, V> Entry<'m, 'k, K, V>
where
    K: WriteXDR,
    V: Store,
{
    pub fn get(&self) -> &Option<V> {
        &self.value
    }

    pub fn get_mut(&mut self) -> &mut Option<V> {
        self.dirty = true;

        &mut self.value
    }

    /// Set the entry to a new value.  The change
    /// will be saved to the backing store when the
    /// `Entry` is dropped.
    pub fn set(&mut self, value: &V)
    where
        V: Clone
    {
        self.value = Some(value.clone());

        self.dirty = true;
    }

    /// Store the current entry value back to its storage location
    /// if it could have changed since being loaded.  (This is
    /// usually not necessary because it is called automatically
    /// when the `Entry` is dropped.)
    fn flush(&mut self) {
        if self.dirty {
            if let Some(value) = self.value.as_ref() {
                self.map.set(self.entry_key, value).unwrap_or_else(|e|
                    panic!("Failed flushing a Map::Entry to the store with Map::set(): {}", e));
            } else {
                self.map.delete(self.entry_key).unwrap_or_else(|e|
                    panic!("Failed flushing a Map::Entry to the store with Map::delete(): {}", e));
            }

            self.dirty = false;
        }
    }
}

impl<'m, 'k, K, V> Drop for Entry<'m, 'k, K, V>
where
    K: WriteXDR,
    V: Store,
{
    fn drop(&mut self) {
        self.flush();
    }
}
