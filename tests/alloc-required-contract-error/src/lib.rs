#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use sdk::*;

#[contract_error]
pub struct BlobError {
    pub blob: Vec<u8>,
}

#[derive(Default)]
pub struct AllocRequiredContractError;

#[contract(throws(BlobError), version = "0.3.0", storage = false)]
impl AllocRequiredContractError {
    pub fn fail() {}
}
