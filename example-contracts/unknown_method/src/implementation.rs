use thruput_sdk::*;

pub struct Defined;

#[contract(client = DefinedClient)]
impl Defined {
    pub fn defined() -> u64 {
        1
    }
}
