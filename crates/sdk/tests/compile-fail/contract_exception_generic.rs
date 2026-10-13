#![allow(unexpected_cfgs)]

use thruput_sdk::*;

#[contract_exception]
pub enum GenericError<T> {
    Inner(T) = 1,
}

fn main() {}
