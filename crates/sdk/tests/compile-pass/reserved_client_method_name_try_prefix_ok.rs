#![allow(unexpected_cfgs)]

use sdk::*;

#[contract]
pub trait Retryable {
    fn retry_transfer() -> u32;
}

fn main() {}
