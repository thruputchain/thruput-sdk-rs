use super::keypath::Keypath;
use super::node::StorageType;
use super::slot::{SlotMut, SlotRef};
use core::fmt::Debug;
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};
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
        V: StorageType,
    {
        let keypath = self.make_entry_keypath(entry_key);
        let slot = V::from(keypath);
        SlotRef::new(self, slot)
    }

    pub fn at_mut(&mut self, entry_key: &K) -> SlotMut<'_, Self, V>
    where
        K: WriteXDR,
        V: StorageType,
    {
        let keypath = self.make_entry_keypath(entry_key);
        let slot = V::from(keypath);
        SlotMut::new(self, slot)
    }

    pub fn at_mut_exclusive(&mut self, entry_key: &K) -> SlotMut<'_, Self, V>
    where
        K: WriteXDR,
        V: StorageType,
    {
        let mut keypath = self.make_entry_keypath(entry_key);
        keypath.set_exclusive();
        let slot = V::from(keypath);
        SlotMut::new(self, slot)
    }

    /// Mutably borrow N elements at once. Panics if any keys are equal.
    ///
    /// Returns a `DisjointMut` guard that borrows the map exclusively
    /// and provides `DerefMut` access to the `[V; N]` array.
    ///
    /// Mirrors `HashMap::get_disjoint_mut`.
    pub fn at_disjoint_mut<const N: usize>(
        &mut self,
        keys: [&K; N],
    ) -> DisjointMut<'_, Self, V, N>
    where
        K: WriteXDR + PartialEq,
        V: StorageType,
    {
        // O(n^2) deduplication check
        for i in 0..N {
            for j in (i + 1)..N {
                assert!(
                    keys[i] != keys[j],
                    "at_disjoint_mut: duplicate keys at indices {i} and {j}"
                );
            }
        }

        let slots = keys.map(|k| V::from(self.make_entry_keypath(k)));
        DisjointMut {
            slots,
            _borrow: PhantomData,
        }
    }

    /// Mutably borrow N elements at once without checking for duplicates.
    ///
    /// # Safety
    ///
    /// The caller must ensure all keys are disjoint. Passing duplicate
    /// keys creates aliased handles to the same storage slot across the
    /// syscall boundary, which may silently corrupt data.
    pub unsafe fn at_disjoint_unchecked_mut<const N: usize>(
        &mut self,
        keys: [&K; N],
    ) -> DisjointMut<'_, Self, V, N>
    where
        K: WriteXDR,
        V: StorageType,
    {
        let slots = keys.map(|k| V::from(self.make_entry_keypath(k)));
        DisjointMut {
            slots,
            _borrow: PhantomData,
        }
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

/// Borrow guard holding N mutable storage slots from a Map.
/// Borrows the map exclusively for its lifetime.
pub struct DisjointMut<'a, Owner, S, const N: usize> {
    slots: [S; N],
    _borrow: PhantomData<&'a mut Owner>,
}

impl<Owner, S, const N: usize> Deref for DisjointMut<'_, Owner, S, N> {
    type Target = [S; N];

    fn deref(&self) -> &[S; N] {
        &self.slots
    }
}

impl<Owner, S, const N: usize> DerefMut for DisjointMut<'_, Owner, S, N> {
    fn deref_mut(&mut self) -> &mut [S; N] {
        &mut self.slots
    }
}

impl<K, V> super::sealed::Sealed for Map<K, V> {}
impl<K, V> super::node::StorageType for Map<K, V> {}
