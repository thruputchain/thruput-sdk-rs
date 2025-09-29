use super::error::Result;
use super::keypath::DefaultFromKeypath;
use super::keypath::Keypath;
use super::load::LoadKeypath;
use core::fmt::Debug;
use core::marker::PhantomData;
use xdrs::WriteXDR;

#[derive(Debug)]
pub struct Map<K, V> {
    keypath: Keypath,
    // This type should be covariant in K and V, but (for drop
    // purposes) doesn't actually own values of either type
    _marker: PhantomData<fn() -> (K, V)>,
}

impl<K, V> From<Keypath> for Map<K, V> {
    fn from(keypath: Keypath) -> Self {
        let _marker = PhantomData;
        Self { keypath, _marker }
    }
}

impl<K, V> DefaultFromKeypath for Map<K, V> {
    fn default_from_keypath(keypath: &Keypath) -> Self {
        Self::new(keypath)
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
        K: WriteXDR,
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
        K: WriteXDR,
    {
        let mut keypath = self.keypath.clone();
        keypath.push_item(entry_key);
        keypath
    }
}

impl_read_xdr_path_via_from! { Map<K, V> }
impl_write_xdr_empty! { Map<K, V> }
