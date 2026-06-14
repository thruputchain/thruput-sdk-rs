#![allow(unexpected_cfgs)]

use sdk::*;

pub struct BadContract;

#[contract]
impl BadContract {
    pub fn bad(_value: fn(u32) -> u32) {}
}

fn main() {}
