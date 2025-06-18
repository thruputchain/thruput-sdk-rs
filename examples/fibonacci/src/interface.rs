use sdk::prelude::*;

pub use sdk::prelude::{Error, Result};

#[contract(client = "client")]
pub trait Fibonacci {
    fn initialize(maybe_self: Option<&mut Self>) -> Result<()>;
    fn fibonacci(&self, n: u32) -> u32;
    fn slow_fibonacci(&self, n: u32) -> u32;
}
