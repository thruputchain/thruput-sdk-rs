#![allow(unexpected_cfgs)]

use thruput_sdk::*;

#[thruput_sdk::storage]
pub struct BareContract;

#[contract]
impl BareContract {
    pub fn owner_byte(&self, owner: &&ContractAddress) -> u32 {
        u32::from(owner.as_ref()[0])
    }
}

fn main() {}
