use thruput_sdk::*;

#[contract(version = "1.2.3")]
pub trait VersionedCallee {
    fn add_one(x: u32) -> u32;
}
