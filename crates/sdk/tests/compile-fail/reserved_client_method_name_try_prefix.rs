use thruput_sdk::*;

#[contract]
pub trait TokenLike {
    fn try_transfer(to: ContractAddress, amount: u64);
}

fn main() {}
