#![allow(unexpected_cfgs)]

use sdk::*;

#[contracttype(name = "SpecPayload")]
pub struct Payload {
    pub count: u32,
}

#[contracttype(name = "SpecChoice")]
pub enum Choice {
    Left = 1,
    Right = 2,
}

fn main() {}
