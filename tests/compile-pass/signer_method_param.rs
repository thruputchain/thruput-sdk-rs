#![allow(unexpected_cfgs)]

use sdk::*;
use sdk::signer::Signer;

#[derive(Default)]
pub struct SignerTestContract;

#[contract(storage = false)]
impl SignerTestContract {
    pub fn with_signer(&self, amount: u64, auth: Signer<u32>) -> u32 {
        let _ = (amount, auth);
        0
    }

    pub fn multiple_signers(
        &self,
        a: Signer<u32>,
        x: i32,
        b: Signer<bool>,
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
