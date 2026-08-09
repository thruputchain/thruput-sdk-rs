#![allow(unexpected_cfgs)]

use sdk::*;

#[sdk::storage]
pub struct BareContract;

#[contract]
impl BareContract {
    pub fn lookup<'a>(&self, key: &'a [u8]) -> u32 {
        key.len() as u32
    }
}

fn main() {}
