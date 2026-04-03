use sdk::*;

pub struct RawGasConfigured;

#[contract]
impl RawGasConfigured {
    pub fn r#with_gas_limit() -> u32 {
        7
    }
}

fn main() {}
