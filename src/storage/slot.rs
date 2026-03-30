// src/storage/slot.rs
use super::node::StorageNode;
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};

/// Read-only borrowed slot. Holds a shared borrow on the parent collection.
pub struct SlotRef<'a, Owner, S: StorageNode> {
    slot: S,
    _borrow: PhantomData<&'a Owner>,
}

impl<'a, Owner, S: StorageNode> SlotRef<'a, Owner, S> {
    pub fn new(_owner: &'a Owner, slot: S) -> Self {
        Self {
            slot,
            _borrow: PhantomData,
        }
    }
}

impl<Owner, S: StorageNode> Deref for SlotRef<'_, Owner, S> {
    type Target = S;

    fn deref(&self) -> &Self::Target {
        &self.slot
    }
}

/// Mutable borrowed slot. Holds an exclusive borrow on the parent collection.
pub struct SlotMut<'a, Owner, S: StorageNode> {
    slot: S,
    _borrow: PhantomData<&'a mut Owner>,
}

impl<'a, Owner, S: StorageNode> SlotMut<'a, Owner, S> {
    pub fn new(_owner: &'a mut Owner, slot: S) -> Self {
        Self {
            slot,
            _borrow: PhantomData,
        }
    }
}

impl<Owner, S: StorageNode> Deref for SlotMut<'_, Owner, S> {
    type Target = S;

    fn deref(&self) -> &Self::Target {
        &self.slot
    }
}

impl<Owner, S: StorageNode> DerefMut for SlotMut<'_, Owner, S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.slot
    }
}
