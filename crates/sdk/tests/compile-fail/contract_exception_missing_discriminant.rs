#![allow(unexpected_cfgs)]

use sdk::contract_exception;

#[contract_exception]
enum Error {
    One = 1,
    Two,
}

fn main() {}
