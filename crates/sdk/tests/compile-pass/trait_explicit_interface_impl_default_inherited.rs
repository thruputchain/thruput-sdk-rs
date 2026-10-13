#![allow(unexpected_cfgs)]
use thruput_sdk::*;

pub struct MyContract;

#[contract(interface = "RenamedInterface")]
pub trait MyTrait {
    fn do_thing() -> u32;
}

#[contract]
impl MyTrait for MyContract {
    fn do_thing() -> u32 {
        42
    }
}

fn main() {}
