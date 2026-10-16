#![allow(unexpected_cfgs)]

#[derive(thruput_sdk::Storage)]
#[storage_domain(foo)]
pub struct S {
    v: thruput_sdk::storage::NNInt,
}

fn main() {}
