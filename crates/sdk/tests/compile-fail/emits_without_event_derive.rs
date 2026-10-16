#![allow(unexpected_cfgs)]

use thruput_sdk::*;

pub struct NotAnEvent {
    pub value: u32,
}

#[contract(emits(NotAnEvent))]
pub trait Token {
    fn ping() -> u32;
}

fn main() {}
