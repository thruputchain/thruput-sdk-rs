#![allow(unexpected_cfgs)]

use sdk::*;

// Both names for the macro may be used:

#[contracttype]
pub struct Payload {
    pub count: u32,
}

#[contract_type]
pub enum Choice {
    Left = 1,
    Right = 2,
}

fn main() {}
