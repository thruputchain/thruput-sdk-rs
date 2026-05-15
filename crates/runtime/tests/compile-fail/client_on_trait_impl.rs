#![allow(unexpected_cfgs)]

use sdk::*;

pub struct MyContract;

#[contract(client = MyClient)]
pub trait MyTrait {
    fn do_thing() -> u32;
}

#[contract(client = BadClient)]
impl MyTrait for MyContract {
    fn do_thing() -> u32 {
        42
    }
}

fn main() {}
