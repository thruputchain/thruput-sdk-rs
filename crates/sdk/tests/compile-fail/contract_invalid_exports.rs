#![allow(unexpected_cfgs)]

use thruput_sdk::*;

#[thruput_sdk::storage]
pub struct BareContract;

#[contract(exports = "123 not an ident")]
impl BareContract {
    pub fn ping(&self) -> u32 {
        7
    }
}

fn main() {}
