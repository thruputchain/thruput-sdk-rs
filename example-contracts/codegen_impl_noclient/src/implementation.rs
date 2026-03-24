use sdk::*;

#[derive(Default)]
pub struct Calc;

#[contract(storage = false, client = CalcClient)]
impl Calc {
    pub fn add(a: u32, b: u32) -> u32 {
        a + b
    }
}
