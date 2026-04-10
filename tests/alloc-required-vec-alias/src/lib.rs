#![allow(unexpected_cfgs)]
#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use sdk::*;

type Blob = Vec<u8>;

#[derive(Default)]
pub struct AllocRequiredVecAlias;

#[contract(version = "0.3.0", storage = false)]
impl AllocRequiredVecAlias {
    pub fn echo(blob: Blob) -> Blob {
        blob
    }
}
