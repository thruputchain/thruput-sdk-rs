#![allow(unexpected_cfgs)]

use thruput_sdk::*;

#[contract]
pub trait Vault {
    fn deposit(&mut self, amount: u128);

    fn version(&self) -> u32 {
        1
    }
}

fn main() {}
