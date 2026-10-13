#![allow(unexpected_cfgs)]

use thruput_sdk::*;

#[contract]
pub trait Retryable {
    fn retry_transfer() -> u32;
}

fn main() {}
