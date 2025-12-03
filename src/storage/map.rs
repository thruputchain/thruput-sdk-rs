use super::error::Result;
use super::keypath::Keypath;
use super::load::LoadKeypath;
use super::store::*;
use crate::delete;
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

        delete(&key)?;

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

        Ok(Entry { entry_key, value, map: self })
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
        &mut self.value
    }

    /// Store a new value into the entry.  This just
    /// calls `set` on the `Map` containing this entry.
    pub fn set(&mut self, value: &V) -> Result<()>
    where
        K: WriteXDR,
        V: Store,
    {
        self.map.set(self.entry_key, value)
    }

    /// Store the current entry value back to its storage location
    pub fn save(&mut self) -> Result<()>
    {
        if let Some(value) = self.value.as_ref() {
            self.map.set(self.entry_key, value)
        } else {
            self.map.delete(self.entry_key)
        }
    }
}

impl<'m, 'k, K, V> Drop for Entry<'m, 'k, K, V>
where
    K: WriteXDR,
    V: Store,
{
    fn drop(&mut self) {
        self.save().unwrap()
    }
}
