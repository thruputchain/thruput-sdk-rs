// A plain value as a storage field. What this pins is the span: the error
// lands on `count`, not on the `#[thruput_sdk::storage]` attribute with
// rustc's `#![feature(trivial_bounds)]` suggestion, which is what a
// where-clause bound on a concrete field type produces and which never says
// which field is wrong.
//
// The message is rustc's own for an unsatisfied `From` bound, and it is not
// good: it offers `From<bool>` and friends and never mentions `Value`.
// Improving it needs a local trait to carry `on_unimplemented`, which would
// narrow what the derive accepts, so the span is all this fixes.
#![allow(unexpected_cfgs)]

use thruput_sdk::ContractAddress;
use thruput_sdk::storage::*;

#[thruput_sdk::storage]
pub struct Config {
    count: u64,
    owner: Value<ContractAddress>,
}

fn main() {}
