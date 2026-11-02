#![no_std]

use thruput_sdk::*;

#[contract_exception]
pub enum ImportedError {
    Denied = 1,
}
