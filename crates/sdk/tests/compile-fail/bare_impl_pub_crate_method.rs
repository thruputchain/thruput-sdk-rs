#![allow(unexpected_cfgs)]

use thruput_sdk::*;

#[thruput_sdk::storage]
pub struct Admin;

#[contract]
impl Admin {
    pub fn ping(&self) -> u32 {
        1
    }

    pub(crate) fn admin_reset(&self) {}

    pub(super) fn admin_promote(&self) {}
}

fn main() {}
