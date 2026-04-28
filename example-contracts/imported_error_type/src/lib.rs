#![no_std]
#![feature(used_with_arg)]

use sdk::*;

#[contract_exception]
pub enum ImportedError {
    Denied = 1,
}
