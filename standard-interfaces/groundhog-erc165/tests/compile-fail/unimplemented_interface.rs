#![allow(unexpected_cfgs)]

use sdk::*;

#[contract]
pub trait Implemented {
    fn implemented(&self) -> u32;
}

#[contract]
pub trait NotImplemented {
    fn not_implemented(&self) -> u32;
}

#[sdk::storage]
pub struct Contract {
    value: sdk::storage::Value<u32>,
}

#[contract]
impl Implemented for Contract {
    fn implemented(&self) -> u32 {
        1
    }
}

// `Contract` implements `Implemented` but not `NotImplemented`: the second
// claim must fail to compile.
groundhog_erc165_if::impl_erc165!(Contract: Implemented, NotImplemented);

fn main() {}
