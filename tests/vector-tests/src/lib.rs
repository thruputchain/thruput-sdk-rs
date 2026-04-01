#![cfg_attr(not(feature = "dev"), no_std)]

use sdk::storage::{Deletable, Map, NNInt, Value, Vector};
use sdk::*;

#[sdk::storage]
pub struct VectorTestContract {
    values: Vector<Value<u64>>,
    nnints: Vector<NNInt>,
    nested: Map<u32, Vector<Value<u64>>>,
}

#[contract(client = VectorTestClient)]
impl VectorTestContract {
    pub fn push_value(&mut self, val: u64) {
        self.values.push_with(|slot| slot.set(&val));
    }

    pub fn get_value(&self, index: u32) -> u64 {
        self.values.at(index).get()
    }

    pub fn get_value_or_zero(&self, index: u32) -> u64 {
        self.values.at(index).try_get().unwrap_or(0)
    }

    pub fn values_len(&self) -> u32 {
        self.values.len()
    }

    pub fn push_nnint(&mut self) {
        self.nnints.push_empty_slot();
    }

    pub fn inc_nnint(&mut self, index: u32, delta: i128) {
        self.nnints.at_mut(index).strict_inc(delta);
    }

    pub fn get_nnint(&self, index: u32) -> i128 {
        self.nnints.at(index).get()
    }

    pub fn nnints_len(&self) -> u32 {
        self.nnints.len()
    }

    pub fn nested_push(&mut self, key: u32, val: u64) {
        self.nested.at_mut(&key).push_with(|slot| slot.set(&val));
    }

    pub fn nested_get(&self, key: u32, index: u32) -> u64 {
        self.nested.at(&key).at(index).get()
    }

    pub fn nested_len(&self, key: u32) -> u32 {
        self.nested.at(&key).len()
    }

    pub fn pop_value(&mut self) {
        self.values.pop();
    }

    pub fn pop_nnint(&mut self) {
        self.nnints.pop();
    }

    pub fn delete_values(&mut self) {
        self.values.delete();
    }

    /// Sum all values using iter() (read-only).
    pub fn sum_values(&self) -> u64 {
        self.values.iter().map(|v| v.get()).sum()
    }

    /// Increment every nnint by `delta` using iter_mut().
    pub fn inc_all_nnints(&mut self, delta: i128) {
        for mut elem in self.nnints.iter_mut() {
            elem.strict_inc(delta);
        }
    }
}
