#![allow(unexpected_cfgs)]

use thruput_sdk::*;

#[contract]
pub trait Vault {
    type Asset;

    fn ping(&self) -> u32;
}

fn main() {}
