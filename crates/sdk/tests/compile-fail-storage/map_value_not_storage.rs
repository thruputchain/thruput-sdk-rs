// A plain value as a `Map` value type. The bound is only checked at the
// first access, so that is where the diagnostic has to be useful.
//
// rustc lists the implementors itself here, because `StorageType` is sealed
// with explicit impls. What the attribute adds is the part it cannot infer:
// that the fix is to wrap the value, not to reach for a different type.
#![allow(unexpected_cfgs)]

use thruput_sdk::ContractAddress;
use thruput_sdk::storage::*;

#[thruput_sdk::storage]
pub struct Balances {
    by_owner: Map<ContractAddress, u64>,
}

pub fn read(b: &Balances, who: &ContractAddress) {
    let _ = b.by_owner.at(who);
}

fn main() {}
