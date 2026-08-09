#![allow(unexpected_cfgs)]

use sdk::*;

#[contract]
pub fn hash_it<T>(value: T) -> u32 {
    let _ = value;
    0
}

fn main() {}
