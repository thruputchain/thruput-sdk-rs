#![allow(unexpected_cfgs)]

mod inner {
    use sdk::*;

    #[derive(Default)]
    pub struct BareContract;

    #[contract(storage = false)]
    impl BareContract {
        pub fn ping(&self) -> u32 {
            7
        }
    }
}

fn main() {
    let _ = inner::BareContractEntrypoints::ping.0;
}
