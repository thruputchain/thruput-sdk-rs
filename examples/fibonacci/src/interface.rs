use sdk::prelude::*;

pub use sdk::prelude::{Error, Result};

#[contract(client = "client")]
pub trait Fibonacci {
    fn initialize() -> Result<()>;
    fn fibonacci(n: u32) -> u32;
    fn slow_fibonacci(n: u32) -> u32;
}
