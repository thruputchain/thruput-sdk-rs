#![allow(unexpected_cfgs)]

use thruput_sdk::*;

#[derive(Event)]
pub struct GenericEvent<T> {
    pub value: T,
}

fn main() {}
