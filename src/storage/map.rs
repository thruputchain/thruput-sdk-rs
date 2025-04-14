use super::keypath::DefaultFromKeypath;
use super::keypath::Keypath;
use super::load::LoadKeypath;
use super::Result;
use crate::serialize::Serialize;
use core::fmt::Debug;
use core::marker::PhantomData;

#[derive(Debug)]
pub struct Map<K, V> {
    keypath: Keypath,
    // This type should be covariant in K and V, but (for drop
    // purposes) doesn't actually own values of either type
    _marker: PhantomData<fn() -> (K, V)>,
}

// TODO: impl Index?

impl<K, V> LoadKeypath for Map<K, V> {
    fn load_keypath(keypath: &Keypath) -> Result<Self> {
        Ok(Map::new(keypath))
    }
}

impl<K, V> DefaultFromKeypath for Map<K, V> {
    fn default_from_keypath(keypath: &Keypath) -> Self {
        Map::new(keypath)
    }
}

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
        K: Serialize,
        V: LoadKeypath,
    {
        let keypath = &self.make_entry_keypath(entry_key);
        LoadKeypath::load_keypath(keypath)
    }

    // TODO: have get() return a shared ref to the entry
    // (itself a shared ref to the map, plus the entry_key),
    // and get_mut() returns a mutable ref?

    fn make_entry_keypath(&self, entry_key: &K) -> Keypath
    where
        K: Serialize,
    {
        let mut keypath = self.keypath.clone();
        keypath.push_item(entry_key);
        keypath
    }
}

serialize_unit_impl!(Map<K, V>);

deserialize_keypath_impl!(Map<K, V>, "a Map<K, V>");
