#![allow(unexpected_cfgs)]

use sdk::*;

#[sdk::storage]
pub struct BareContract;

#[contract]
impl BareContract {
    pub fn owner_byte(&self, owner: &&ContractAddress) -> u32 {
        u32::from(owner.as_ref()[0])
    }
}

fn main() {}
