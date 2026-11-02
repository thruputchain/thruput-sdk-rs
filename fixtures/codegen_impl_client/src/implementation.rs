use thruput_sdk::*;

#[derive(Default)]
pub struct Calc;

#[contract(client = CalcProxy)]
impl Calc {
    pub fn add(a: u32, b: u32) -> u32 {
        a + b
    }
}
