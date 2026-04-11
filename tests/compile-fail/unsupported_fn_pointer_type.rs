#![allow(unexpected_cfgs)]

use sdk::*;

#[derive(Default)]
pub struct BadContract;

#[contract(storage = false)]
impl BadContract {
    pub fn bad(_value: fn(u32) -> u32) {}
}

fn main() {}
