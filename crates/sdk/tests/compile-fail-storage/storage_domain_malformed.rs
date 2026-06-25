#![allow(unexpected_cfgs)]

#[derive(sdk::Storage)]
#[storage_domain(foo)]
pub struct S {
    v: sdk::storage::NNInt,
}

fn main() {}
