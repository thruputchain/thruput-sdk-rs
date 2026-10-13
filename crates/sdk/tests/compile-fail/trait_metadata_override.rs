#![allow(unexpected_cfgs)]

#[thruput_sdk::contract]
pub trait Interface {
    fn ping() -> u32;
}

pub struct Contract;

#[thruput_sdk::contract]
impl Interface for Contract {
    const __GROUNDHOG_ENTRYPOINT_ping: u64 = 0;
    fn ping() -> u32 {
        42
    }
}

fn main() {}
