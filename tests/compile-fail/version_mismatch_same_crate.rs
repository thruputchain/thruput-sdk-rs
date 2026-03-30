use sdk::*;

pub struct MyContract;

#[contract]
pub trait MyTrait {
    fn do_thing() -> u32;
}

#[contract(version = "99.0.0")]
impl MyTrait for MyContract {
    fn do_thing() -> u32 {
        42
    }
}

fn main() {}
