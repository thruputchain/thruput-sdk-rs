#![allow(unexpected_cfgs)]

use thruput_sdk::*;

#[thruput_sdk::storage]
pub struct BareContract;

#[contract]
impl BareContract {
    pub fn bump(&self, counter: &mut u32) -> u32 {
        *counter += 1;
        *counter
    }
}

fn main() {}
