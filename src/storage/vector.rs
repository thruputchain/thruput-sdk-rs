use super::keypath::Keypath;
use super::map::Map;
use super::node::StorageType;
use super::slot::{SlotMut, SlotRef};
use super::value::Value;
use core::fmt::Debug;

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

impl_load_via_keypath! { Vector<S> }
impl_read_via_keypath! { Vector<S> }

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
