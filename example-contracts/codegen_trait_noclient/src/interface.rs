use sdk::*;

#[contract]
pub trait Adder {
    fn add(a: u32, b: u32) -> u32;
}
