#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();

use sdk::*;
use sdk::signer::Signer;

#[contracttype]
pub struct TransferAuth {
    pub amount: u64,
}

#[derive(Default)]
pub struct SignerCallee;

#[contract(storage = false)]
impl SignerCallee {
    pub fn accept_signer(auth: Signer<TransferAuth>) -> u32 {
        auth.handle()
    }

    pub fn accept_mixed(x: u32, auth: Signer<TransferAuth>, y: u32) -> u32 {
        x + y + auth.handle()
    }
}
