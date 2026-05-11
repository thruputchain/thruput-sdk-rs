#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();

use sdk::signer::Signed;
use sdk::*;

#[contracttype]
pub struct LeafAuth {
    pub data: u32,
}

#[derive(Default)]
pub struct SignerLeaf;

#[contract(storage = false)]
impl SignerLeaf {
    pub fn consume_leaf(auth: Signed<LeafAuth>) -> u32 {
        let leaf: LeafAuth = auth.consume().0;
        leaf.data
    }
}
