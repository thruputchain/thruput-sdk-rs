use crate::VersionedCallee;
use sdk::*;

pub struct VersionedCalleeContract;

#[contract(version = "1.2.3")]
impl VersionedCallee for VersionedCalleeContract {
    fn add_one(x: u32) -> u32 {
        x + 1
    }
}
