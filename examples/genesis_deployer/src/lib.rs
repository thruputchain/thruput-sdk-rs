#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use sdk::{MethodID, SHA256Hash, sys};

pub struct GenesisDeployer;

impl GenesisDeployer {
    pub fn deploy(self, nonce: &SHA256Hash, contract_hash: &SHA256Hash) {
        let _ = sdk::deploy_contract(nonce, contract_hash).unwrap();
    }

    pub fn deploy_and_invoke(
        self,
        nonce: &SHA256Hash,
        contract_hash: &SHA256Hash,
        method: MethodID,
        calldata: Vec<u8>,
    ) {
        let deployed_addr =
            sdk::deploy_contract(nonce, contract_hash).unwrap();
        let flags = 0;

        let mut return_len = 0;
        sys::raw_Invoke(
            &deployed_addr,
            method,
            u64::MAX,
            calldata.as_slice(),
            flags,
            &mut return_len,
        )
        .unwrap();
    }
}
