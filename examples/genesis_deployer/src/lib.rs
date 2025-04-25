#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use sdk::prelude::*;
use sdk::sys;
use sdk::types::MethodID;

pub struct GenesisDeployer;

impl GenesisDeployer {
    pub fn deploy(self, nonce: &SHA256Hash, contract_hash: &SHA256Hash) {
        let _ = deploy_contract(nonce, contract_hash).unwrap();
    }

    pub fn deploy_and_invoke(
        self,
        nonce: &SHA256Hash,
        contract_hash: &SHA256Hash,
        method: MethodID,
        calldata: Vec<u8>,
    ) {
        let deployed_addr = deploy_contract(nonce, contract_hash).unwrap();

        let mut return_len = 0;
        sys::raw_Invoke(
            &deployed_addr,
            method,
            u64::MAX,
            calldata.as_slice(),
            &mut return_len,
        )
        .unwrap();
    }
}
