#![allow(unexpected_cfgs)]

use sdk::*;

pub struct BadContract;

#[contract]
impl BadContract {
    pub fn bad(_value: impl core::fmt::Debug) {}
}

fn main() {}
