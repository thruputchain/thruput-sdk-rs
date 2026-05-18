use sdk::*;

#[contract]
pub trait TokenLike {
    fn r#try_transfer(to: ContractAddress, amount: u64);
}

fn main() {}
