#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();

extern crate alloc;

use alloc::vec::Vec;
use sdk::transaction::InvocationFrameStatusCode;
use sdk::{SHA256Hash, sys, *};

pub struct GenesisDeployer;

#[contract(client = GenesisDeployerClient)]
impl GenesisDeployer {
    pub fn deploy(nonce: &SHA256Hash, contract_hash: &SHA256Hash) {
        let _ = sdk::deploy_contract(nonce, contract_hash);
    }

    pub fn deploy_and_invoke(
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
            reserved: 0,
        };
        let mut outputs = sys::CopyableInvokeOutputs {
            return_buffer_len: 0,
            returned_auth_token_count: 0,
            // Placeholder; overwritten by the syscall on Ok(). Using Fault
            // (not Return) ensures an unwritten field is caught by the
            // assert_eq below rather than silently passing.
            frame_status: InvocationFrameStatusCode::Fault,
        };
        sys::raw_Invoke(
            &deployed_addr,
            calldata.as_slice(),
            &[],
            &inputs,
            &mut outputs,
        )
        .unwrap();
        // A child abort/fault is no longer surfaced as a syscall error; keep
        // genesis fail-closed by requiring the invoked frame to have returned.
        assert_eq!(
            outputs.frame_status,
            InvocationFrameStatusCode::Return,
            "genesis deploy_and_invoke: invoked frame did not return"
        );
    }
}
