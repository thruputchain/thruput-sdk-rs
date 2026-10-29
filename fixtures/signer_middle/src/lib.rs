#![cfg_attr(not(feature = "dev"), no_std)]

extern crate alloc;

thruput_sdk::declare_mock_contract!();

use alloc::vec::Vec;
use signer_leaf::{LeafAuth, SignerLeafClient};
use thruput_sdk::signer::Signed;
use thruput_sdk::*;

#[contract_type]
pub struct MiddleAuth {
    pub label: u64,
    pub child: Signed<LeafAuth>,
    pub extra_children: Vec<Signed<LeafAuth>>,
}

pub struct SignerMiddle;

#[contract]
impl SignerMiddle {
    pub fn consume_and_forward(
        auth: Signed<MiddleAuth>,
        leaf_contract: &ContractAddress,
    ) -> u64 {
        let middle: MiddleAuth = auth.consume().0;

        let child_result =
            SignerLeafClient::new(leaf_contract).consume_leaf(middle.child);

        let mut extra_sum: u64 = 0;
        for child in middle.extra_children {
            extra_sum += SignerLeafClient::new(leaf_contract)
                .consume_leaf(child) as u64;
        }

        middle.label + child_result as u64 + extra_sum
    }
}
