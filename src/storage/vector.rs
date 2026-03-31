use super::keypath::Keypath;
use super::map::Map;
use super::nnint::NNInt;
use super::node::StorageNode;
use super::slot::{SlotMut, SlotRef};
use core::fmt::Debug;

pub struct Vector<S> {
    keypath: Keypath,
    len: NNInt,
    values: Map<u64, S>,
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
            len: NNInt::from(len_kp),
            values: Map::from(values_kp),
        }
    }
}

impl_from_keypath! { Vector<S> }
impl_load_via_keypath! { Vector<S> }
impl_read_via_keypath! { Vector<S> }

impl<S: StorageNode> StorageNode for Vector<S> {
    fn keypath(&self) -> &Keypath {
        &self.keypath
    }
}

impl<S: StorageNode> Vector<S> {
    /// Current length (number of elements). Returns 0 if never written.
    pub fn len(&self) -> u64 {
        self.len.try_get().unwrap_or(0) as u64
    }

    /// Borrow element at `index` (bounds-checked).
    pub fn at(&self, index: u64) -> SlotRef<'_, Self, S> {
        self.bounds_check(index);
        let keypath = self.make_elem_keypath(index);
        let slot = S::from_keypath(keypath);
        SlotRef::new(self, slot)
    }

    /// Mutably borrow element at `index` (bounds-checked).
    pub fn at_mut(&mut self, index: u64) -> SlotMut<'_, Self, S> {
        self.bounds_check(index);
        let keypath = self.make_elem_keypath(index);
        let slot = S::from_keypath(keypath);
        SlotMut::new(self, slot)
    }

    /// Allocate a new slot at the end and return a mutable reference to it.
    pub fn push(&mut self) -> SlotMut<'_, Self, S> {
        let idx = self.len();
        self.len.set((idx + 1) as i128);
        let keypath = self.make_elem_keypath(idx);
        let slot = S::from_keypath(keypath);
        SlotMut::new(self, slot)
    }

    fn bounds_check(&self, index: u64) {
        let len = self.len();
        if index >= len {
            super::error::oob_throw(len, index);
        }
    }

    fn make_elem_keypath(&self, index: u64) -> Keypath {
        let mut kp = self.values.keypath().clone();
        kp.push_item(&index);
        kp
    }
}
