#![allow(unexpected_cfgs)]

use thruput_sdk::contract_type;

#[contract_type]
pub enum Action {
    Noop = 0,
    Delegated(u32),
}

fn main() {}
