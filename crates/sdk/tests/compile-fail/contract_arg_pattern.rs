#![allow(unexpected_cfgs)]

use sdk::*;

#[sdk::storage]
pub struct BareContract;

#[contract]
impl BareContract {
    pub fn ping(&self, _: u32) -> u32 {
        7
    }
}

fn main() {}
