#![allow(unexpected_cfgs)]

use thruput_sdk::*;
use thruput_sdk::signer::Signed;

#[thruput_sdk::storage]
pub struct SignerTestContract;

#[contract]
impl SignerTestContract {
    pub fn with_signer(&self, amount: u64, auth: Signed<u32>) -> u32 {
        let _ = (amount, auth);
        0
    }

    pub fn multiple_signers(
        &self,
        a: Signed<u32>,
        x: i32,
        b: Signed<bool>,
    ) -> u32 {
        let _ = (a, x, b);
        0
    }

    pub fn no_signers(&self, x: u64) -> u64 {
        x
    }
}

fn main() {
    let client = SignerTestContractClient::new(&ContractAddress::default());
    let _ = &client;
}
