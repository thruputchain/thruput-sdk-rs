use thruput_sdk::*;

#[contract]
pub trait RawConstructorLike {
    fn r#new() -> u32;
}

fn main() {}
