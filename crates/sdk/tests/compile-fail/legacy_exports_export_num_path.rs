#![allow(unexpected_cfgs)]
use sdk::*;

#[sdk::storage]
pub struct LegacyPathContract;

#[contract]
impl LegacyPathContract {
    pub fn ping(&self) -> u32 {
        7
    }
}

fn main() {
    let _ = exports::PING_EXPORT_NUM;
}
