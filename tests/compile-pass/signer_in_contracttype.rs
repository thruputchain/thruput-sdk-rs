#![allow(unexpected_cfgs)]

use sdk::*;
use sdk::signer::Signer;

#[contracttype]
pub struct TransferRequest {
    pub amount: u64,
    pub auth: Signer<u32>,
}

#[derive(Default)]
pub struct SignerStructContract;

#[contract(storage = false)]
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
