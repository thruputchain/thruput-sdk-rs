// A plain value as a `Map` value type. The bound is only checked at the
// first access, so that is where the diagnostic has to be useful: it must
// say to wrap the value, not list the nine implementors.
#![allow(unexpected_cfgs)]

use sdk::ContractAddress;
use sdk::storage::*;

#[sdk::storage]
pub struct Balances {
    by_owner: Map<ContractAddress, u64>,
}

pub fn read(b: &Balances, who: &ContractAddress) {
    let _ = b.by_owner.at(who);
}

fn main() {}
