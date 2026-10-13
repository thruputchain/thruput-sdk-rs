#![allow(unexpected_cfgs)]

use thruput_sdk::*;
use version_mismatch_helper::VersionedTrait;

pub struct MyContract;

#[contract]
impl VersionedTrait for MyContract {
    fn versioned_method() -> u32 {
        42
    }
}

fn main() {}
