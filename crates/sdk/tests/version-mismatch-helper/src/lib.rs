#![allow(clippy::result_large_err)]

use thruput_sdk::*;

#[contract(client = VersionedClient)]
pub trait VersionedTrait {
    fn versioned_method() -> u32;
}
