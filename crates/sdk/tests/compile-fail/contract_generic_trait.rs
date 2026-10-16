#![allow(unexpected_cfgs)]

use thruput_sdk::*;

#[contract]
pub trait Vault<T> {
    fn deposit(&mut self, asset: T);
}

fn main() {}
