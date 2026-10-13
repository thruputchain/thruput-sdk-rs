#![allow(unexpected_cfgs)]

use thruput_sdk::*;

#[thruput_sdk::storage]
pub struct BareContract;

#[contract]
impl BareContract {
    pub fn ping(&self, _: u32) -> u32 {
        7
    }
}

fn main() {}
