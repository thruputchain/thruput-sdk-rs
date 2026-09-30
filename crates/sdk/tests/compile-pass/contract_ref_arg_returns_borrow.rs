#![allow(unexpected_cfgs)]

//! A free-function entrypoint can return a borrow of a `&T` parameter: the
//! decoded arguments live until the result is encoded.

use sdk::*;

#[sdk::contract]
pub fn echo(data: &[u8]) -> &[u8] {
    data
}

#[sdk::contract]
pub fn echo_text(text: &str) -> &str {
    text
}

#[sdk::contract]
pub fn echo_address(address: &ContractAddress) -> &ContractAddress {
    address
}

/// Borrows from the local the slice was decoded into, not from calldata.
#[sdk::contract]
pub fn first_word(words: &[u32]) -> &u32 {
    &words[0]
}

fn main() {}
