use sdk::*;

#[contract]
pub trait Retryable {
    fn retry_transfer() -> u32;
}

fn main() {}
