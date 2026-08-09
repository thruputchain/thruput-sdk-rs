#![allow(unexpected_cfgs)]

use sdk::*;

#[contract]
pub trait Vault {
    type Asset;

    fn ping(&self) -> u32;
}

fn main() {}
