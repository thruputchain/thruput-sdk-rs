//! Deploys a wasm contract and makes an invocation on it, via native_exec

use sdk::prelude::*;
use xdrs::BytesM;
use thiserror::Error;
use sdk::contract_result::Result as ContractResult;
use sdk::types::transaction::Invocation;
use sdk::{sys, contract_panic};

#[derive(Error, Debug)]
pub enum Error {
    #[error("IO error")]
    IO(#[from] std::io::Error),
    // sdk::contract_result::ContractError;
}

type Result<T, E = Error> = core::result::Result<T, E>;

pub fn exec_path(wasm_path: &str, inv: &Invocation) -> Result<()> {
    let wasm_bytes = std::fs::read(wasm_path)?;

    exec(wasm_bytes, inv)
}

// We'd like to return the resulting return-data
/// Deploy the wasm contract, replace the invocation address
/// with the deployed contract, and perform the invocation on it.
pub fn exec(wasm_bytes: Vec<u8>, inv: &Invocation) -> Result<()> {
    let contract_blob: ContractBlob = BytesM(wasm_bytes);
    let contract = Contract::kWasmi {
        wasmi_contract: contract_blob,
    };

    let mut env = sdk::TestEnvironment::new();
    sdk::TestEnvironment::enable_syscall_logging();

    let contract_hash = env.setup_create_contract(&contract);
    let inv = inv.clone();

    let mut handler = env.prepare_test(move || -> () {
        // We want to capture the return data here
        match exec_by_hash(contract_hash, &inv) {
            Ok(buf) => panic!("guest contract succeeded: {buf:?}"),
            Err(e) => panic!("guest contract failed: {e}"),
        }
    });

    let res = handler.root_invoke();
    assert_eq!(res.result.panic, sdk::InvokeErrorCode::kInvokeNone as i32);

    Ok(())
}

fn exec_by_hash(contract_hash: SHA256Hash, inv: &Invocation) -> ContractResult<Vec<u8>> {

    // Deploy the contract to be tested
    let nonce = SHA256Hash::from([8; 32]);
    let deployed_addr = deploy_contract(&nonce, &contract_hash)?;

    let entrypoint = &inv.entrypoint;
    let entrypoint_u64 = u64::from_be_bytes(entrypoint.into_inner());
    let mut return_len = 0;

    let result = sys::raw_Invoke(
        &deployed_addr,
        entrypoint_u64,
        inv.gas_limit,
        &inv.calldata,
        0, // TODO(geoff): invoke flags go here
        &mut return_len,
    );

    let is_ok = match result {
        Ok(()) => true,
        Err(syserr) => {
            if syserr.is_aborted() {
                false
            } else {
                // Some other error: fail now
                return Err(InvokeError::Syscall(syserr).into())
            }
        }
    };

    // Ok or abort: get the return buffer
    let mut return_buf = vec![0; return_len as usize];
    let mut written: RuntimeMemoryLength = 0;
    sys::raw_GetReturnedBuffer(return_buf.as_mut_slice(), &mut written)
        .map_err(InvokeError::Syscall)?;
    if written != return_len {
        contract_panic!("unexpected return buffer length");
    }

    if is_ok {
        Ok(return_buf)
    } else {
        Err(InvokeError::Aborted(return_buf).into())
    }
}
