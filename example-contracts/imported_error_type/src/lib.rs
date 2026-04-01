#![no_std]

use sdk::*;

#[contract_error]
pub enum ImportedError {
    Denied = 1,
}
