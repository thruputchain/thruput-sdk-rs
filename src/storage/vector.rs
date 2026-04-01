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
        len_kp.set_exclusive();

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
        let new_len = idx.checked_add(1).expect("vector length overflow");
        self.len.set(&new_len);
        let keypath = self.make_elem_keypath(idx);
        let slot = S::from(keypath);
        SlotMut::new(self, slot)
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
    /// Remove the last element, deleting its storage with an exclusive lock
    /// (matching the exclusivity of the length update).
    /// Aborts if the vector is empty.
    pub fn pop(&mut self) {
        let len = self.len();
        if len == 0 {
            super::error::oob_throw(0, 0);
        }
        let new_len = len - 1;
        let mut elem = S::from(self.make_elem_keypath(new_len));
        elem.delete_internal(StorageDeltaFlags::ExclusiveLock);
        self.len.set(&new_len);
    }
}

impl<S: StorageType + Deletable> Deletable for Vector<S> {
    // Unlike pop(), which hard-codes ExclusiveLock on the element delete,
    // delete_internal propagates the caller's flags to elements. The length
    // key's own exclusive keypath flag is OR'd in by Value::delete_internal,
    // so `vec.delete()` uses shared locking for elements but exclusive for
    // the length. Use `vec.delete_exclusive()` for fully exclusive teardown.
    fn delete_internal(&mut self, flags: StorageDeltaFlags) {
        let len = self.len();
        for i in 0..len {
            let mut elem = S::from(self.make_elem_keypath(i));
            elem.delete_internal(flags);
        }
        self.len.delete_internal(flags);
    }
}
