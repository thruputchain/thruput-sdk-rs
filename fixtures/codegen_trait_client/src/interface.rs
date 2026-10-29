use thruput_sdk::*;

#[contract(client = AdderProxy)]
pub trait Adder {
    fn add(a: u32, b: u32) -> u32;
}
