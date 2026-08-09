#![allow(unexpected_cfgs)]

use sdk::*;

#[contract_exception]
pub enum GenericError<T> {
    Inner(T) = 1,
}

fn main() {}
