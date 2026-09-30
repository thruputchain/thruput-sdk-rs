#![allow(unexpected_cfgs)]
use sdk::*;
use version_mismatch_helper::VersionedTrait as RenamedTrait;

pub struct MyContract;

#[contract(interface = "RenamedTrait")]
impl RenamedTrait for MyContract {
    fn versioned_method() -> u32 {
        42
    }
}

fn main() {}
