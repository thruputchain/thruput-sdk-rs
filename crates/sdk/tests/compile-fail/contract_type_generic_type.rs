use thruput_sdk::*;

#[contract_type]
pub struct Wrapper<T> {
    pub value: T,
}

fn main() {}
