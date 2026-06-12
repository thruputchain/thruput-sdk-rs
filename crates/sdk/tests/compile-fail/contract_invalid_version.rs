#![allow(unexpected_cfgs)]

use sdk::*;

#[sdk::storage]
pub struct BareContract;

#[contract(version = "two")]
impl BareContract {
    pub fn ping(&self) -> u32 {
        7
    }
}

fn main() {}
