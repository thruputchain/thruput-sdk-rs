use super::deletable::Deletable;
use super::keypath::{FramedComponent, Keypath};
use super::map::Map;
use super::node::{FromConstPath, StorageType};
use super::slot::{SlotMut, SlotRef};
use super::value::Value;
use crate::exn::NoneIfMissing;
use core::fmt::Debug;
use groundhog_xdr::storage_item::StorageDeltaFlags;

/// Element slots are keyed by the element index (`u32`); see [`Map`] for the
/// key-encoding injectivity invariant that governs the underlying derivation.
pub struct Vector<S> {
    // A bare Keypath, not a KeyCache: the vector's own slot is never read
    // or written (elements live under `values`, the length under `len`),
    // so there is no key hash to memoize — an OnceCell here would be dead
    // weight in every instance. The path is retained only for
    // path_bytes()/Debug. It is always an Owned-style path: the const-path
    // constructor below seeds it from the const bytes.
    keypath: Keypath,
    len: Value<u32>,
    values: Map<u32, S>,
}

impl<S> Debug for Vector<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Vector")
            .field("keypath", &self.keypath.path_bytes())
            .field("len", &self.len)
            .finish_non_exhaustive()
    }
}

impl<S> From<Keypath> for Vector<S> {
    fn from(keypath: Keypath) -> Self {
        // Fixed layout slots (0 = len, 1 = values), pre-framed at const
        // time: state-load appends constant bytes instead of runtime-CXS-
        // encoding through the scratch buffer. Element slots stay runtime-
        // derived (indices are dynamic).
        const LEN_SLOT: FramedComponent = FramedComponent::from_u32(0);
        const VALUES_SLOT: FramedComponent = FramedComponent::from_u32(1);
        let len_kp = keypath.child_framed(&LEN_SLOT);
        let values_kp = keypath.child_framed(&VALUES_SLOT);

        Self {
            keypath,
            len: Value::from(len_kp),
            values: Map::from(values_kp),
        }
    }
}

impl<S> super::sealed::Sealed for Vector<S> {}
impl<S> super::node::StorageType for Vector<S> {}

impl<S: StorageType> FromConstPath for Vector<S> {
    /// Construct a `Vector<S>` from a `'static` const preimage path.
    ///
    /// `Vector` has internal sub-fields (`len`, `values`) that need
    /// their own derived paths; this uses `Keypath::from_bytes` to seed
    /// a runtime Keypath, then delegates to `From<Keypath>` — one copy
    /// of the prefix bytes, then today's sub-field construction.
    fn __sdk_from_const_path(path: &'static [u8]) -> Self {
        Self::from(Keypath::from_bytes(path))
    }
}

impl<S> Vector<S> {
    /// Raw preimage bytes of this vector's storage path.
    ///
    /// Useful for path-identity comparisons (e.g. drift-guard tests).
    pub fn path_bytes(&self) -> &[u8] {
        self.keypath.path_bytes()
    }
}

impl<S: StorageType> Vector<S> {
    /// Current logical extent (number of in-bounds indices).
    ///
    /// A missing len slot reads as 0 (a never-written vector is empty).
    /// Any other failure — a value at the slot that is not a `u32`, a
    /// wrong storage type, a host fault — aborts typed like every other
    /// storage accessor, instead of masquerading as an empty vector.
    ///
    /// Vector slots are sparse-capable: callers may delete element storage
    /// without shrinking len, so an in-bounds index does not guarantee that
    /// the slot currently has backing data.
    pub fn len(&self) -> u32 {
        self.len.try_get().none_if_missing().unwrap_or(0)
    }

    /// Returns true if the vector contains no elements.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Borrow element at `index` (bounds-checked).
    pub fn at(&self, index: u32) -> SlotRef<'_, Self, S> {
        self.bounds_check(index);
        let keypath = self.make_elem_keypath(index);
        let slot = S::from(keypath);
        SlotRef::new(self, slot)
    }

    /// Mutably borrow element at `index` (bounds-checked).
    pub fn at_mut(&mut self, index: u32) -> SlotMut<'_, Self, S> {
        self.bounds_check(index);
        let keypath = self.make_elem_keypath(index);
        let slot = S::from(keypath);
        SlotMut::new(self, slot)
    }

    /// Allocate a new sparse-capable slot at the end and return a mutable
    /// handle to it.
    ///
    /// This increments len before the returned slot is written, making the new
    /// index immediately in-bounds for the rest of the invocation.
    #[must_use = "push_empty_slot increments len immediately; dropping the slot leaves it uninitialized. Use push_with to initialize atomically, or bind with `let _ = ...` if an empty slot is intentional."]
    pub fn push_empty_slot(&mut self) -> SlotMut<'_, Self, S> {
        let idx = self.len();
        let new_len = idx.checked_add(1).unwrap();
        self.len.set_exclusive(&new_len);
        let keypath = self.make_elem_keypath(idx);
        let slot = S::from(keypath);
        SlotMut::new(self, slot)
    }

    /// Construct a new slot at the end, initialize it through `f`, then make
    /// the index visible by incrementing len.
    ///
    /// If `f` panics or traps, the entire invocation frame faults and reverts,
    /// so neither the slot data nor the len increment are committed.
    pub fn push_with<R>(&mut self, f: impl FnOnce(&mut S) -> R) -> R {
        let idx = self.len();
        let mut slot = S::from(self.make_elem_keypath(idx));
        let out = f(&mut slot);
        let new_len = idx.checked_add(1).unwrap();
        self.len.set_exclusive(&new_len);
        out
    }

    /// Iterate over all elements. Yields read-only `SlotRef` handles.
    /// Holds a shared borrow on the vector, preventing structural
    /// modifications (`push_with`/`push_empty_slot`/pop/at_mut) while the
    /// iterator is alive.
    pub fn iter(&self) -> VectorIter<'_, S> {
        VectorIter {
            vec: self,
            index: 0,
            len: self.len(),
        }
    }

    /// Iterate over all elements with mutable access. Yields owned
    /// storage handles that may be used for mutations. Holds an
    /// exclusive borrow on the vector.
    pub fn iter_mut(&mut self) -> VectorIterMut<'_, S> {
        let len = self.len();
        VectorIterMut {
            vec: self,
            index: 0,
            len,
        }
    }

    fn bounds_check(&self, index: u32) {
        let len = self.len();
        if index >= len {
            super::error::oob_throw(len as u64, index as u64);
        }
    }

    fn make_elem_keypath(&self, index: u32) -> Keypath {
        self.values.make_entry_keypath(&index)
    }
}

impl<S: StorageType + Deletable> Vector<S> {
    /// Remove the last element, deleting its storage.
    /// Aborts if the vector is empty.
    pub fn pop(&mut self) {
        let len = self.len();
        if len == 0 {
            super::error::oob_throw(0, 0);
        }
        let new_len = len - 1;
        let mut elem = S::from(self.make_elem_keypath(new_len));
        elem.delete_internal(StorageDeltaFlags::ExclusiveLock);
        self.len.set_exclusive(&new_len);
    }
}

impl<S: StorageType + Deletable> Deletable for Vector<S> {
    fn delete_internal(&mut self, flags: StorageDeltaFlags) {
        let len = self.len();
        for i in 0..len {
            let mut elem = S::from(self.make_elem_keypath(i));
            elem.delete_internal(flags);
        }
        self.len.delete_internal(flags);
    }
}

/// Iterator over vector elements (shared borrow, read-only).
pub struct VectorIter<'a, S> {
    vec: &'a Vector<S>,
    index: u32,
    len: u32,
}

impl<'a, S: StorageType> Iterator for VectorIter<'a, S> {
    type Item = SlotRef<'a, Vector<S>, S>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.len {
            return None;
        }
        let elem = S::from(self.vec.make_elem_keypath(self.index));
        self.index += 1;
        Some(SlotRef::new(self.vec, elem))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = (self.len - self.index) as usize;
        (remaining, Some(remaining))
    }
}

impl<S: StorageType> ExactSizeIterator for VectorIter<'_, S> {}

impl<'a, S: StorageType> DoubleEndedIterator for VectorIter<'a, S> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.index >= self.len {
            return None;
        }
        self.len -= 1;
        let elem = S::from(self.vec.make_elem_keypath(self.len));
        Some(SlotRef::new(self.vec, elem))
    }
}

/// Iterator over vector elements (exclusive borrow).
pub struct VectorIterMut<'a, S> {
    vec: &'a mut Vector<S>,
    index: u32,
    len: u32,
}

impl<'a, S: StorageType> Iterator for VectorIterMut<'a, S> {
    type Item = SlotMut<'a, Vector<S>, S>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.len {
            return None;
        }
        let elem = S::from(self.vec.make_elem_keypath(self.index));
        self.index += 1;
        // SAFETY: the iterator holds &'a mut Vector<S>, ensuring
        // exclusive access to the vector for 'a.
        Some(SlotMut::new_detached(elem))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = (self.len - self.index) as usize;
        (remaining, Some(remaining))
    }
}

impl<S: StorageType> ExactSizeIterator for VectorIterMut<'_, S> {}

impl<'a, S: StorageType> DoubleEndedIterator for VectorIterMut<'a, S> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.index >= self.len {
            return None;
        }
        self.len -= 1;
        let elem = S::from(self.vec.make_elem_keypath(self.len));
        Some(SlotMut::new_detached(elem))
    }
}

impl<'a, S: StorageType> IntoIterator for &'a Vector<S> {
    type Item = SlotRef<'a, Vector<S>, S>;
    type IntoIter = VectorIter<'a, S>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, S: StorageType> IntoIterator for &'a mut Vector<S> {
    type Item = SlotMut<'a, Vector<S>, S>;
    type IntoIter = VectorIterMut<'a, S>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

#[cfg(test)]
mod test_vector_slots {
    use super::*;
    use crate::storage::Value;

    /// Drift guard: the const-framed fixed slots (0 = len, 1 = values)
    /// must address exactly what runtime child() derives.
    #[test]
    fn fixed_slots_match_runtime_child() {
        let root = Keypath::default().child("V");
        let v: Vector<Value<u32>> = Vector::from(root.clone());
        assert_eq!(v.len.path_bytes(), root.child(&0u32).path_bytes());
        assert_eq!(v.values.path_bytes(), root.child(&1u32).path_bytes());
        assert_eq!(v.path_bytes(), root.path_bytes());
    }
}
