#![cfg_attr(not(feature = "dev"), no_std)]

use sdk::storage::{Map, NNInt, Value, Vector};
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
        self.values.push().set(&val);
    }

    pub fn get_value(&self, index: u64) -> u64 {
        self.values.at(index).get()
    }

    pub fn get_value_or_zero(&self, index: u64) -> u64 {
        self.values.at(index).try_get().unwrap_or(0)
    }

    pub fn values_len(&self) -> u64 {
        self.values.len()
    }

    pub fn push_nnint(&mut self) {
        self.nnints.push();
    }

    pub fn inc_nnint(&mut self, index: u64, delta: i128) {
        self.nnints.at_mut(index).strict_inc(delta);
    }

    pub fn get_nnint(&self, index: u64) -> i128 {
        self.nnints.at(index).get()
    }

    pub fn nnints_len(&self) -> u64 {
        self.nnints.len()
    }

    pub fn nested_push(&mut self, key: u32, val: u64) {
        self.nested.at_mut(&key).push().set(&val);
    }

    pub fn nested_get(&self, key: u32, index: u64) -> u64 {
        self.nested.at(&key).at(index).get()
    }

    pub fn nested_len(&self, key: u32) -> u64 {
        self.nested.at(&key).len()
    }
}
