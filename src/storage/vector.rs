use super::deletable::Deletable;
use super::keypath::Keypath;
use super::map::Map;
use super::node::StorageType;
use super::slot::{SlotMut, SlotRef};
use super::value::Value;
use core::fmt::Debug;
use groundhog_xdr::storage_item::StorageDeltaFlags;

pub struct Vector<S> {
    keypath: Keypath,
    len: Value<u32>,
    values: Map<u32, S>,
}

impl<S> Debug for Vector<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Vector")
            .field("keypath", &self.keypath)
            .field("len", &self.len)
            .finish_non_exhaustive()
    }
}

impl<S> From<Keypath> for Vector<S> {
    fn from(keypath: Keypath) -> Self {
        let mut len_kp = keypath.clone();
        len_kp.seq_begin();

        let mut values_kp = keypath.clone();
        values_kp.seq_begin();
        values_kp.seq_next();

        Self {
            keypath,
            len: Value::from(len_kp),
            values: Map::from(values_kp),
        }
    }
}

impl<S> super::sealed::Sealed for Vector<S> {}
impl<S> super::node::StorageType for Vector<S> {}

impl<S> Vector<S> {
    pub fn keypath(&self) -> &Keypath {
        &self.keypath
    }
}

impl<S: StorageType> Vector<S> {
    /// Current length (number of elements). Returns 0 if never written.
    pub fn len(&self) -> u32 {
        self.len.try_get().unwrap_or(0)
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

    /// Allocate a new slot at the end and return a mutable reference to it.
    pub fn push(&mut self) -> SlotMut<'_, Self, S> {
        let idx = self.len();
        let new_len = idx.checked_add(1).unwrap();
        self.len.set_exclusive(&new_len);
        let keypath = self.make_elem_keypath(idx);
        let slot = S::from(keypath);
        SlotMut::new(self, slot)
    }

    /// Iterate over all elements. Yields read-only `SlotRef` handles.
    /// Holds a shared borrow on the vector, preventing structural
    /// modifications (push/pop/at_mut) while the iterator is alive.
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
        let mut kp = self.values.keypath().clone();
        kp.push_item(&index);
        kp
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
