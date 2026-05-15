use sdk::*;

pub trait ExternalTokenLike {
    fn try_transfer(&self, to: ContractAddress, amount: u64);
}

pub struct TokenContract;

#[contract]
impl ExternalTokenLike for TokenContract {
    fn try_transfer(&self, to: ContractAddress, amount: u64) {
        let _ = (to, amount);
    }
}

fn main() {}
