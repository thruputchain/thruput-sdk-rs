#![allow(unexpected_cfgs)]

use thruput_sdk::*;

#[thruput_sdk::storage]
pub struct BareContract;

#[contract(interface = "My-Interface")]
impl BareContract {
    pub fn ping(&self) -> u32 {
        7
    }
}

fn main() {}
