#![allow(unexpected_cfgs)]

use thruput_sdk::*;

#[derive(Debug, PartialEq)]
pub struct NotAnExn {
    pub code: u32,
}

#[contract(throws = NotAnExn)]
pub trait Token {
    fn ping() -> u32;
}

fn main() {}
