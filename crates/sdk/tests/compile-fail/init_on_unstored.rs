#![allow(unexpected_cfgs)]

use sdk::*;

pub struct MyContract;

#[contract]
impl MyContract {
    #[init]
    fn do_thing() -> u32 {
        42
    }
}

fn main() {}
