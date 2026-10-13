use thruput_sdk::*;

trait ErrorTrait {
    type Assoc;
}

#[contract(throws = <Thing as ErrorTrait>::Assoc)]
pub trait ThrowsQSelfTrait {
    fn ping();
}

fn main() {}
