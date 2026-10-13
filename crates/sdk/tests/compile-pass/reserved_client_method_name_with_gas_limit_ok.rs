#![allow(unexpected_cfgs)]

use thruput_sdk::*;

pub struct GasConfigured;

#[contract]
impl GasConfigured {
    pub fn with_gas_limit_extra() -> u32 {
        7
    }
}

fn main() {}
