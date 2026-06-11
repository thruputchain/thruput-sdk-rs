#![allow(unexpected_cfgs)]

use sdk::*;

#[derive(Default)]
pub struct BareContract;

#[contract(storage = false, interface = "My-Interface")]
impl BareContract {
    pub fn ping(&self) -> u32 {
        7
    }
}

fn main() {}
