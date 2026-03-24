use super::Adder;
use sdk::*;

#[derive(Default)]
struct AdderContract;

#[contract(storage = false)]
impl Adder for AdderContract {
    fn add(a: u32, b: u32) -> u32 {
        a + b
    }
}
