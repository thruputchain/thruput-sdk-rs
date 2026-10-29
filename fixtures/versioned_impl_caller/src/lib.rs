//! Calls both versions of the `Adder` interface on one deployed
//! `versioned_impl` from inside a guest, so both clients' encodings and the
//! two distinct entrypoint IDs are exercised contract-to-contract.

#![cfg_attr(not(feature = "dev"), no_std)]

thruput_sdk::declare_mock_contract!();

use adder_v1::AdderClient as AdderV1Client;
use adder_v2::AdderClient as AdderV2Client;
use thruput_sdk::*;

pub struct VersionedImplCaller;

#[contract(client = VersionedImplCallerClient)]
impl VersionedImplCaller {
    /// Alternate calls between the 0.1 and 0.2 `Adder` interfaces of the
    /// contract at `target`, trapping on any wrong result.
    pub fn run(target: &ContractAddress) {
        let v1 = AdderV1Client::new(target);
        let v2 = AdderV2Client::new(target);
        assert!(v1.add_one(41) == 42);
        // This value cannot be encoded by the 0.1 interface's u32 ABI.
        let large = u64::from(u32::MAX) + 41;
        assert!(v2.add_one(large) == large + 1);
        assert!(v1.add_one(99) == 100);
    }
}
