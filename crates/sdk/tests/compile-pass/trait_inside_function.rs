#![allow(unexpected_cfgs)]
#![deny(non_local_definitions)]

fn main() {
    #[sdk::contract]
    pub trait Local {
        fn ping() -> u32;
    }
}
