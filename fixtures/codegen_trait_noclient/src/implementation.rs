use super::Adder;
use thruput_sdk::*;

#[derive(Default)]
struct AdderContract;

#[contract]
impl Adder for AdderContract {
    fn add(a: u32, b: u32) -> u32 {
        a + b
    }
}
