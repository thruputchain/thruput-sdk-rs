#![no_std]

use sdk::*;

#[contract_exception]
pub enum ImportedError {
    Denied = 1,
}
