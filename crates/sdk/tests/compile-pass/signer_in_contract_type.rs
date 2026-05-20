#![allow(unexpected_cfgs)]

use sdk::*;
use sdk::signer::Signed;

#[contract_type]
pub struct TransferRequest {
    pub amount: u64,
    pub auth: Signed<u32>,
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
