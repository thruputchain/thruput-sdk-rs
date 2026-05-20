#![allow(unexpected_cfgs)]

use sdk::*;

#[contract_type(name = "SpecPayload")]
pub struct Payload {
    pub count: u32,
}

#[contract_type(name = "SpecChoice")]
pub enum Choice {
    Left = 1,
    Right = 2,
}

fn main() {}
