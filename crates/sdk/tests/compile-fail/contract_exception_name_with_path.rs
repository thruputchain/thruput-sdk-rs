#![allow(unexpected_cfgs)]

// `#[contract_exception]` names its type through the same spec id as
// `#[contract_type]`, so it refuses a `::` in `name` the same way.
#[sdk::contract_exception(name = "spelled::Out")]
pub enum MyError {
    One = 1,
}

fn main() {}
