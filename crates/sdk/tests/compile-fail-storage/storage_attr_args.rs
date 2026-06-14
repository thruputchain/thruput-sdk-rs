#![allow(unexpected_cfgs)]

#[sdk::storage(anything = "here")]
pub struct S {
    v: sdk::storage::NNInt,
}

fn main() {}
