#![cfg_attr(not(feature = "dev"), no_std)]

thruput_sdk::declare_mock_contract!();

use thruput_sdk::signer::Signed;
use thruput_sdk::*;

#[contract_type]
pub struct LeafAuth {
    pub data: u32,
}

pub struct SignerLeaf;

#[contract]
impl SignerLeaf {
    pub fn consume_leaf(auth: Signed<LeafAuth>) -> u32 {
        let leaf: LeafAuth = auth.consume().0;
        leaf.data
    }
}
