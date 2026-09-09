#![allow(unexpected_cfgs)]

#[sdk::contract_exception(name = "inner::MyError")]
pub enum MyError {
    One = 1,
}

fn main() {}
