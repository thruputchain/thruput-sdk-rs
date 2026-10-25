use super::deletable::Deletable;
use super::keypath::Keypath;
use super::map::DisjointMut;
use super::node::StorageType;
use super::slot::{SlotMut, SlotRef};
use core::fmt::Debug;
use core::marker::PhantomData;

/// A fixed-length array of `N` storage slots of type `S`.
///
/// Element `i` lives at the array's keypath extended by `i` as a `u32` — the
/// same slot [`Map<u32, S>`](super::Map) at that keypath would use for key
/// `i`, so the two layouts are interchangeable. Like `Map`, an `Array` holds
/// only its own keypath and derives an element's key when the element is
/// accessed, so its size and load cost do not depend on `N`. Unlike `Map`, the
/// length is known at compile time: indexing is bounds-checked without a
/// storage read, and the array can enumerate (iterate and delete) its
/// elements.
///
/// Unlike [`Vector`](super::Vector), there is no stored length: every index
/// in `0..N` is always in bounds, and an element that was never written (or
/// was deleted) reads back as missing. Read with `try_get` +
/// [`none_if_missing`](crate::exn::NoneIfMissing::none_if_missing) when an
/// element may be unset. Elements share no storage, so writes to distinct
/// elements never conflict with each other.
///
/// `N` must fit in a `u32`; a larger `N` fails to compile on first use.
pub struct Array<S, const N: usize> {
    keypath: Keypath,
    _marker: PhantomData<fn() -> S>,
}

impl<S, const N: usize> Debug for Array<S, N> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Array")
            .field("keypath", &self.keypath)
            .field("len", &N)
            .finish()
    }
}

impl<S, const N: usize> From<Keypath> for Array<S, N> {
    fn from(keypath: Keypath) -> Self {
        const { assert!(N <= u32::MAX as usize, "Array length must fit in a u32") };
        let _marker = PhantomData;
        Self { keypath, _marker }
    }
}

impl<S, const N: usize> super::sealed::Sealed for Array<S, N> {}
impl<S, const N: usize> super::node::StorageType for Array<S, N> {}

impl<S, const N: usize> Array<S, N> {
    /// The array's length, `N`.
    pub const LEN: u32 = N as u32;

    pub fn keypath(&self) -> &Keypath {
        &self.keypath
    }

    /// The array's length, `N`. Never reads storage.
    pub const fn len(&self) -> u32 {
        Self::LEN
    }

    /// Returns true if `N == 0`.
    pub const fn is_empty(&self) -> bool {
        N == 0
    }
}

impl<S: StorageType, const N: usize> Array<S, N> {
    /// Borrow element at `index` (bounds-checked).
    pub fn at(&self, index: u32) -> SlotRef<'_, Self, S> {
        let slot = S::from(self.make_elem_keypath(index));
        SlotRef::new(self, slot)
    }

    /// Mutably borrow element at `index` (bounds-checked).
    pub fn at_mut(&mut self, index: u32) -> SlotMut<'_, Self, S> {
        let slot = S::from(self.make_elem_keypath(index));
        SlotMut::new(self, slot)
    }

    /// Like [`at_mut`](Self::at_mut), but every successful write through the
    /// returned handle is followed by a request for an exclusive lock on the
    /// element. See [`Map::at_mut_exclusive`](super::Map::at_mut_exclusive).
    pub fn at_mut_exclusive(&mut self, index: u32) -> SlotMut<'_, Self, S> {
        let mut keypath = self.make_elem_keypath(index);
        keypath.set_exclusive();
        let slot = S::from(keypath);
        SlotMut::new(self, slot)
    }

    /// Mutably borrow M elements at once (bounds-checked). Panics if any
    /// indices are equal.
    ///
    /// See [`Map::at_disjoint_mut`](super::Map::at_disjoint_mut).
    pub fn at_disjoint_mut<const M: usize>(
        &mut self,
        indices: [u32; M],
    ) -> DisjointMut<'_, Self, S, M> {
        for i in 0..M {
            for j in (i + 1)..M {
                assert!(
                    indices[i] != indices[j],
                    "at_disjoint_mut: duplicate indices at positions {i} and {j}"
                );
            }
        }
        let slots =
            indices.map(|index| S::from(self.make_elem_keypath(index)));
        DisjointMut::new(slots)
    }

    /// Iterate over all `N` elements. Yields read-only `SlotRef` handles,
    /// deriving each element's key as it is reached.
    pub fn iter(&self) -> ArrayIter<'_, S, N> {
        ArrayIter {
            arr: self,
            index: 0,
            end: Self::LEN,
        }
    }

    /// Iterate over all `N` elements with mutable access. Holds an exclusive
    /// borrow on the array.
    pub fn iter_mut(&mut self) -> ArrayIterMut<'_, S, N> {
        ArrayIterMut {
            arr: self,
            index: 0,
            end: Self::LEN,
        }
    }

    fn make_elem_keypath(&self, index: u32) -> Keypath {
        if index >= Self::LEN {
            super::error::oob_throw(N as u64, index as u64);
        }
        self.keypath.child(&index)
    }
}

impl<S: StorageType + Deletable, const N: usize> Array<S, N> {
    /// Delete the storage of the element at `index` (bounds-checked). The
    /// element reads back as missing afterwards.
    #[inline(always)]
    pub fn delete_at(&mut self, index: u32) {
        self.at_mut(index).delete();
    }
}

impl<S: StorageType + Deletable, const N: usize> Deletable for Array<S, N> {
    fn delete_internal(&mut self, exclusive: bool) {
        for i in 0..Self::LEN {
            let mut elem = S::from(self.make_elem_keypath(i));
            elem.delete_internal(exclusive);
        }
    }
}

/// Iterator over array elements (shared borrow, read-only).
pub struct ArrayIter<'a, S, const N: usize> {
    arr: &'a Array<S, N>,
    index: u32,
    end: u32,
}

impl<'a, S: StorageType, const N: usize> Iterator for ArrayIter<'a, S, N> {
    type Item = SlotRef<'a, Array<S, N>, S>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.end {
            return None;
        }
        let elem = S::from(self.arr.make_elem_keypath(self.index));
        self.index += 1;
        Some(SlotRef::new(self.arr, elem))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = (self.end - self.index) as usize;
        (remaining, Some(remaining))
    }
}

impl<S: StorageType, const N: usize> ExactSizeIterator
    for ArrayIter<'_, S, N>
{
}

impl<S: StorageType, const N: usize> DoubleEndedIterator
    for ArrayIter<'_, S, N>
{
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.index >= self.end {
            return None;
        }
        self.end -= 1;
        let elem = S::from(self.arr.make_elem_keypath(self.end));
        Some(SlotRef::new(self.arr, elem))
    }
}

/// Iterator over array elements (exclusive borrow).
pub struct ArrayIterMut<'a, S, const N: usize> {
    arr: &'a mut Array<S, N>,
    index: u32,
    end: u32,
}

impl<'a, S: StorageType, const N: usize> Iterator for ArrayIterMut<'a, S, N> {
    type Item = SlotMut<'a, Array<S, N>, S>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.end {
            return None;
        }
        let elem = S::from(self.arr.make_elem_keypath(self.index));
        self.index += 1;
        // The iterator holds &'a mut Array<S, N>, ensuring exclusive access
        // to the array for 'a.
        Some(SlotMut::new_detached(elem))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = (self.end - self.index) as usize;
        (remaining, Some(remaining))
    }
}

impl<S: StorageType, const N: usize> ExactSizeIterator
    for ArrayIterMut<'_, S, N>
{
}

impl<S: StorageType, const N: usize> DoubleEndedIterator
    for ArrayIterMut<'_, S, N>
{
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.index >= self.end {
            return None;
        }
        self.end -= 1;
        let elem = S::from(self.arr.make_elem_keypath(self.end));
        Some(SlotMut::new_detached(elem))
    }
}

impl<'a, S: StorageType, const N: usize> IntoIterator for &'a Array<S, N> {
    type Item = SlotRef<'a, Array<S, N>, S>;
    type IntoIter = ArrayIter<'a, S, N>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, S: StorageType, const N: usize> IntoIterator for &'a mut Array<S, N> {
    type Item = SlotMut<'a, Array<S, N>, S>;
    type IntoIter = ArrayIterMut<'a, S, N>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

#[cfg(test)]
mod test_array_slots {
    use super::*;
    use crate::storage::{Map, Value};

    // Layout guard: element i addresses exactly the slot a Map<u32, _> at the
    // same keypath uses for key i.
    crate::contract_test!(elements_match_map_u32_slots {
        let root = Keypath::default().child("A");
        let arr: Array<Value<u32>, 4> = Array::from(root.clone());
        let map: Map<u32, Value<u32>> = Map::from(root);
        for i in 0..4u32 {
            assert_eq!(*arr.at(i).key(), *map.at(&i).key());
        }
    });
}
