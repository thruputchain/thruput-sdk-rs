#![allow(unexpected_cfgs)]

use sdk::*;

#[derive(Event)]
pub struct GenericEvent<T> {
    pub value: T,
}

fn main() {}
