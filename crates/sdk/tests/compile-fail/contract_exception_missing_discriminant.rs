#![allow(unexpected_cfgs)]

use thruput_sdk::contract_exception;

#[contract_exception]
enum Error {
    One = 1,
    Two,
}

fn main() {}
