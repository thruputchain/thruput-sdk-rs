#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use sdk::{SHA256Hash, sys};

pub struct GenesisDeployer;

impl GenesisDeployer {
    pub fn deploy(self, nonce: &SHA256Hash, contract_hash: &SHA256Hash) {
        let _ = sdk::deploy_contract(nonce, contract_hash);
    }

    pub fn deploy_and_invoke(
        self,
        nonce: &SHA256Hash,
        contract_hash: &SHA256Hash,
        method: u64,
        calldata: Vec<u8>,
    ) {
        let deployed_addr = sdk::deploy_contract(nonce, contract_hash);
        let flags = 0;

        let inputs = sys::CopyableInvokeInputs {
            entrypoint_id: method,
            gas_limit: u64::MAX,
            flags,
        };
        let mut outputs = sys::CopyableInvokeOutputs {
            return_buffer_len: 0,
            returned_auth_token_len: 0,
        };
        sys::raw_Invoke(
            &deployed_addr,
            calldata.as_slice(),
            &[],
            &inputs,
            &mut outputs,
        )
        .unwrap();
    }
}
