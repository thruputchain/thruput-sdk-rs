use sdk::*;
use version_mismatch_helper::VersionedTrait as RenamedTrait;

pub struct MyContract;

#[contract(version = "1.0.0")]
impl RenamedTrait for MyContract {
    fn versioned_method() -> u32 {
        42
    }
}

fn main() {}
