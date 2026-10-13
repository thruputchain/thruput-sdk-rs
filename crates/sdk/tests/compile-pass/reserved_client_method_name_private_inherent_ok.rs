#![allow(unexpected_cfgs)]

use thruput_sdk::*;

pub struct PrivateMethodContract;

#[contract]
impl PrivateMethodContract {
    fn new() -> u32 {
        7
    }
}

fn main() {}
