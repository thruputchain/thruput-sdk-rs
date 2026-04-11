#![no_std]

extern crate alloc;

use sdk::*;

#[contracttype]
pub struct Payload {
    pub count: u32,
}

#[derive(Default)]
pub struct BareTypeContract;

#[contract(version = "0.1.0")]
impl BareTypeContract {
    pub fn echo(payload: Payload) -> Payload {
        payload
    }
}
