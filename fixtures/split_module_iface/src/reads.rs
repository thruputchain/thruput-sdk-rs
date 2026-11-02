//! Half of the `Split` interface. The other half is in `writes`.

use thruput_sdk::*;

#[contract(interface = "Split", version = "0.1.0")]
pub fn peek(value: u32) -> u32 {
    value
}
