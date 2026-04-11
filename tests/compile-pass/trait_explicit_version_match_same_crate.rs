#![allow(unexpected_cfgs)]

use sdk::*;

pub struct MyContract;

#[contract(version = "1.2.3")]
pub trait MyTrait {
    fn do_thing() -> u32;
}

// suffices for the major version to match
#[contract(version = "1.2.4")]
impl MyTrait for MyContract {
    fn do_thing() -> u32 {
        42
    }
}

fn main() {}
