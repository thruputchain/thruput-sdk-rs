use sdk::*;

#[contract]
pub trait Retryable {
    fn retry_transfer() -> u32;
}

pub struct GasConfigured;

#[contract]
impl GasConfigured {
    pub fn with_gas_limit_extra() -> u32 {
        7
    }
}

fn main() {}
