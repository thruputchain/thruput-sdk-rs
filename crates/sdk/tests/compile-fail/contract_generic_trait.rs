#![allow(unexpected_cfgs)]

use sdk::*;

#[contract]
pub trait Vault<T> {
    fn deposit(&mut self, asset: T);
}

fn main() {}
