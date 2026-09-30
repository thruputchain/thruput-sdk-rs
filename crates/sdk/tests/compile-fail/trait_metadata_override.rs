#![allow(unexpected_cfgs)]

#[sdk::contract]
pub trait Interface {
    fn ping() -> u32;
}

pub struct Contract;

#[sdk::contract]
impl Interface for Contract {
    const __GROUNDHOG_ENTRYPOINT_ping: u64 = 0;
    fn ping() -> u32 {
        42
    }
}

fn main() {}
