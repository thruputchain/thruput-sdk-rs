use super::keypath::Keypath;
use super::node::StorageType;
use super::slot::{SlotMut, SlotRef};
use core::fmt::Debug;
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};
use xdrs::WriteXDR;

/// A persistent map from keys of type `K` to storage values of type `V`.
///
/// # Key identity is the XDR encoding
///
/// An entry's storage slot is addressed by `sha256(XDR-encode(key))` — the
/// key's [`WriteXDR`] encoding *is* its identity. Two keys address the same
/// entry if and only if they encode to the same bytes; the in-memory `K` value
/// (and any `PartialEq`/`Eq`/`Hash` it has) is never consulted by the storage
/// layer. In particular, no `Map` operation consults `K`'s `PartialEq`/`Eq`:
/// even the duplicate-key check in [`at_disjoint_mut`](Self::at_disjoint_mut)
/// compares the derived storage keypaths, not the keys themselves.
///
/// # Key encoding invariant
///
/// A key's storage slot is `sha256` of its framed `WriteXDR` encoding, so the
/// encoding **is** the key's identity. Key types MUST encode injectively:
/// distinct keys must produce distinct bytes. All SDK-provided and
/// `#[derive(XDR)]` types satisfy this. A hand-written, non-canonical
/// `WriteXDR` where `a != b` but `encode(a) == encode(b)` will silently alias
/// the two keys onto one slot.
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
    ///
    /// The duplicate-key check compares the derived storage keypaths — the
    /// actual slot identity — so it does not consult `K`'s `PartialEq` and is
    /// exact regardless of how (or whether) `K` implements it.
    pub fn at_disjoint_mut<const N: usize>(
        &mut self,
        keys: [&K; N],
    ) -> DisjointMut<'_, Self, V, N>
    where
        K: WriteXDR,
        V: StorageType,
    {
        let keypaths = keys.map(|k| self.make_entry_keypath(k));

        // O(n^2) check for keys that resolve to the same storage slot.
        for i in 0..N {
            // `i < N <= usize::MAX`, so this cannot wrap; saturating keeps
            // the pair loop from silently degenerating if it ever could,
            // since a skipped pair here means an *undetected* alias.
            for j in i.saturating_add(1)..N {
                assert!(
                    !keypaths[i].addresses_same_slot(&keypaths[j]),
                    "at_disjoint_mut: duplicate keys at indices {i} and {j}"
                );
            }
        }

        let slots = keypaths.map(V::from);
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
        self.keypath.child(entry_key)
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
