#![allow(unexpected_cfgs)]

// Parsing discards the comment; splicing does not. Without an exact-match
// check this would embed the `::` the validator exists to reject.
#[sdk::contract_type(name = "Meta /*::*/")]
pub struct Meta {
    pub count: u32,
}

fn main() {}
