use super::error::Result;
use super::keypath::Keypath;
use super::load::LoadKeypath;
use super::store::*;
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
        let keypath = &self.make_entry_keypath(entry_key);

        LoadKeypath::load_keypath(keypath)
    }

    pub fn set(&self, entry_key: &K, value: &V) -> Result<()>
    where
        K: WriteXDR,
        V: Store,
    {
        let mut keypath = self.keypath().clone();
        keypath.push_item(entry_key);
        let key = keypath.generate_key();

        value.store(&key)
    }

    /*
    pub fn entry<'m, 'k>(
        &'m self,
        key: &'k K,
    ) -> Result<Entry<'m, 'k, K, V>>
    where
        K: WriteXDR,
        V: LoadKeypath,
    {
        let keypath = &self.make_entry_keypath(key);
        let value = LoadKeypath::load_keypath(keypath)?;

        Ok(Entry { key, value, map: PhantomData })
    }
    */

    pub fn entry<'m, 'k>(
        &'m mut self,
        entry_key: &'k K,
    ) -> Result<EntryMut<'m, 'k, K, V>>
    where
        K: WriteXDR,
        V: LoadKeypath + Store,
    {
        let keypath = &self.make_entry_keypath(entry_key);
        let value = LoadKeypath::load_keypath(keypath)?;
        let key = keypath.generate_key();

        Ok(EntryMut { key, value, map: self })
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

use core::ops::{Deref, DerefMut};

#[derive(Clone)]
pub struct Entry<'m, 'k, K, V> {
    key: &'k K,
    value: V,
    map: PhantomData<&'m Map<K, V>>
}

impl<'m, 'k, K, V> Deref for Entry<'m, 'k, K, V> {
    type Target = V;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

pub struct EntryMut<'m, 'k, K, V>
where
    K: WriteXDR,
    V: Store,
{
    key: &'k K,
    value: V,
    map: &'m Map<K, V>,
}

impl<'m, 'k, K, V> EntryMut<'m, 'k, K, V>
where
    K: WriteXDR,
    V: Store,
{
    /// Store the entry value back to its storage location
    pub fn save(&self) -> Result<()>
    where
        K: WriteXDR,
        V: Store,
    {
        self.map.set(self.key, &self.value)
    }
}

impl<'m, 'k, K, V> Drop for EntryMut<'m, 'k, K, V>
where
    K: WriteXDR,
    V: Store,
{
    fn drop(&mut self) {
        self.save().unwrap()
    }
}

impl<'m, 'k, K, V> Deref for EntryMut<'m, 'k, K, V>
where
    K: WriteXDR,
    V: Store,
{
    type Target = V;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<'m, 'k, K, V> DerefMut for EntryMut<'m, 'k, K, V>
where
    K: WriteXDR,
    V: Store,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}
