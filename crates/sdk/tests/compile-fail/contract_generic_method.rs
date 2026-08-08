#![allow(unexpected_cfgs)]

use sdk::*;

#[sdk::storage]
pub struct BareContract;

#[contract]
impl BareContract {
    pub fn store<T>(&mut self, value: T) {
        let _ = value;
    }
}

fn main() {}
