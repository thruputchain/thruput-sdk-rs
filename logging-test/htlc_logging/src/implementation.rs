use crate::HTLC;
use sdk::*;

use sdk::storage::{Map, Value};

#[contract_type]
struct HTLCEntry {
    pub sender: ContractAddress,
    pub receiver: ContractAddress,
    pub token: ContractAddress,
    pub amount: i128,
    pub expiration_height: BlockHeight,
}

#[storage]
struct HTLCContract {
    htlcs: Map<SHA256Hash, Value<HTLCEntry>>,
}

#[contract]
impl HTLC for HTLCContract {
    fn initialize(&mut self) {}

    fn create_htlc(
        &mut self,
        receiver: &ContractAddress,
        token: &ContractAddress,
        amount: i128,
        expiration_height: BlockHeight,
        hash: &SHA256Hash,
    ) {
        let htlc = HTLCEntry {
            sender: sdk::get_caller(),
            receiver: receiver.clone(),
            token: token.clone(),
            amount,
            expiration_height: expiration_height.clone(),
        };

        self.htlcs.at_mut_exclusive(hash).set(&htlc);
    }

    fn claim_htlc(&mut self, _hash: &SHA256Hash, _preimage: &SHA256Hash) {}

    fn refund_htlc(&mut self, _hash: &SHA256Hash) -> () {}
}
