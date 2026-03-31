use super::keypath::Keypath;
use super::node::StorageNode;
use super::slot::{SlotMut, SlotRef};
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
impl_from_keypath! { Map<K, V> }
impl_read_via_keypath! { Map<K, V> }

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

    pub fn at(&self, entry_key: &K) -> SlotRef<'_, Self, V>
    where
        K: WriteXDR,
        V: StorageNode,
    {
        let keypath = self.make_entry_keypath(entry_key);
        let slot = V::from_keypath(keypath);
        SlotRef::new(self, slot)
    }

    pub fn at_mut(&mut self, entry_key: &K) -> SlotMut<'_, Self, V>
    where
        K: WriteXDR,
        V: StorageNode,
    {
        let keypath = self.make_entry_keypath(entry_key);
        let slot = V::from_keypath(keypath);
        SlotMut::new(self, slot)
    }

    pub fn at_mut_exclusive(&mut self, entry_key: &K) -> SlotMut<'_, Self, V>
    where
        K: WriteXDR,
        V: StorageNode,
    {
        let mut keypath = self.make_entry_keypath(entry_key);
        keypath.set_exclusive();
        let slot = V::from_keypath(keypath);
        SlotMut::new(self, slot)
    }

    fn make_entry_keypath(&self, entry_key: &K) -> Keypath
    where
        K: WriteXDR,
    {
        let mut keypath = self.keypath.clone();
        keypath.push_item(entry_key);
        keypath
    }
}

impl<K, V> super::node::StorageNode for Map<K, V> {
    fn keypath(&self) -> &super::keypath::Keypath {
        &self.keypath
    }
}

impl<K, V> super::sealed::Sealed for Map<K, V> {}
impl<K, V> super::node::StorageType for Map<K, V> {}
