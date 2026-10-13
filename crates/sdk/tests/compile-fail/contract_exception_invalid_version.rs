#![allow(unexpected_cfgs)]

#[thruput_sdk::contract_exception(version = "1.2")]
pub enum MyError {
    One = 1,
}

fn main() {}
