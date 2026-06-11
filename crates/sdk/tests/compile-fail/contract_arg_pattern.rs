#![allow(unexpected_cfgs)]

use sdk::*;

#[derive(Default)]
pub struct BareContract;

#[contract(storage = false)]
impl BareContract {
    pub fn ping(&self, _: u32) -> u32 {
        7
    }
}

fn main() {}
