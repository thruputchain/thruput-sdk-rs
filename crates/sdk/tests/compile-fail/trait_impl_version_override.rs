#![allow(unexpected_cfgs)]

use sdk::*;

#[contract(version = "1.2.3")]
pub trait MyTrait {
    fn do_thing() -> u32;
}

pub struct Matching;

#[contract(version = "1.2.3")]
impl MyTrait for Matching {
    fn do_thing() -> u32 {
        42
    }
}

pub struct Mismatching;

#[contract(version = "9.0.0")]
impl MyTrait for Mismatching {
    fn do_thing() -> u32 {
        42
    }
}

fn main() {}
