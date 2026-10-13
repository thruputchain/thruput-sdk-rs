#![allow(unexpected_cfgs)]

mod inner {
    use thruput_sdk::*;

    #[thruput_sdk::storage]
    pub struct BareContract;

    #[contract]
    impl BareContract {
        pub fn ping(&self) -> u32 {
            7
        }
    }
}

fn main() {
    let _ = inner::BareContractEntrypoints::ping.0;
}
