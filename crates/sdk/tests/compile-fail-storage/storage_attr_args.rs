#![allow(unexpected_cfgs)]

#[thruput_sdk::storage(anything = "here")]
pub struct S {
    v: thruput_sdk::storage::NNInt,
}

fn main() {}
