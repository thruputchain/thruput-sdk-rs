use sdk::*;

#[contract]
pub trait InvalidInterface {
    fn new() -> u32;
    fn try_transfer(to: ContractAddress, amount: u64);
}

fn main() {}
