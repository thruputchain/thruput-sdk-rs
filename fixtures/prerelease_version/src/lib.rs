//! Pre-release interface version example.
//!
//! The `#[contract]` version override accepts semver pre-release/build
//! suffixes; the spec pipeline must accept them end to end.  Historically the
//! macro accepted `0.9.0-rc.1` but spec-tool's ingestion rejected anything
//! other than bare MAJOR.MINOR.PATCH, so a pre-release contract compiled and
//! then failed at spec rewrite.  gnaw's `fixtures:specs` task asserts this
//! contract's spec publishes the pre-release version verbatim.
//!
//! (For entrypoint IDs the pre-release suffix is irrelevant: only the
//! major version is hashed, so `0.9.0-rc.1` dispatches as major `0.9`.)

#![cfg_attr(not(feature = "dev"), no_std)]

thruput_sdk::declare_mock_contract!();

use thruput_sdk::*;

pub struct PrereleaseContract;

#[contract(version = "0.9.0-rc.1")]
impl PrereleaseContract {
    pub fn ping() -> u32 {
        9
    }
}
