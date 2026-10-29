use crate::VersionedCallee;
use thruput_sdk::*;

pub struct VersionedCalleeContract;

#[contract]
impl VersionedCallee for VersionedCalleeContract {
    fn add_one(x: u32) -> u32 {
        x + 1
    }
}
