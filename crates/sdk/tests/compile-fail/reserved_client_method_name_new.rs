use thruput_sdk::*;

#[contract]
pub trait ConstructorLike {
    fn new() -> u32;
}

fn main() {}
