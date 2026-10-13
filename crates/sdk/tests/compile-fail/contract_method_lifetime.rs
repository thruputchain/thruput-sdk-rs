#![allow(unexpected_cfgs)]

use thruput_sdk::*;

#[thruput_sdk::storage]
pub struct BareContract;

#[contract]
impl BareContract {
    pub fn lookup<'a>(&self, key: &'a [u8]) -> u32 {
        key.len() as u32
    }
}

fn main() {}
