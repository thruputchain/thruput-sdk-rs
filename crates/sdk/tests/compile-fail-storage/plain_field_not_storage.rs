// A plain value as a storage field. The diagnostic must name the field and
// say to wrap it in `Value<T>`, not report a bare `From<Keypath>` bound at
// the attribute with rustc's `#![feature(trivial_bounds)]` suggestion.
#![allow(unexpected_cfgs)]

use sdk::ContractAddress;
use sdk::storage::*;

#[sdk::storage]
pub struct Config {
    count: u64,
    owner: Value<ContractAddress>,
}

fn main() {}
