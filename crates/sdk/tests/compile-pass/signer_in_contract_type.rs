#![allow(unexpected_cfgs)]

use thruput_sdk::*;
use thruput_sdk::signer::Signed;

#[contract_type]
pub struct TransferRequest {
    pub amount: u64,
    pub auth: Signed<u32>,
}

#[thruput_sdk::storage]
pub struct SignerStructContract;

#[contract]
impl SignerStructContract {
    pub fn transfer(&self, req: TransferRequest) -> u32 {
        let _ = req;
        0
    }
}

fn main() {
    let _client =
        SignerStructContractClient::new(&ContractAddress::default());
}
