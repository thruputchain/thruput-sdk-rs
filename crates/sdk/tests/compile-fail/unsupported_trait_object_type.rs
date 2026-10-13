#![allow(unexpected_cfgs)]

use thruput_sdk::*;

pub struct BadContract;

#[contract]
impl BadContract {
    pub fn bad(_value: &dyn core::fmt::Debug) {}
}

fn main() {}
