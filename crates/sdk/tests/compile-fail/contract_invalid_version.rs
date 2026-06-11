#![allow(unexpected_cfgs)]

use sdk::*;

#[derive(Default)]
pub struct BareContract;

#[contract(storage = false, version = "two")]
impl BareContract {
    pub fn ping(&self) -> u32 {
        7
    }
}

fn main() {}
