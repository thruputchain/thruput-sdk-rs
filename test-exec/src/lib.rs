//! Deploys a wasm contract and makes an invocation on it, via native_exec

use sdk::prelude::*;
use xdrs::BytesM;
use thiserror::Error;
use sdk::types::transaction::Calldata;
use sdk::{sys, contract_panic};

/// A result from a test execution
pub type Result<T, E = Error> = core::result::Result<T, E>;

/// An error from a test execution
#[derive(Error, Debug)]
pub enum Error {
    #[error("IO error")]
    IO(#[from] std::io::Error),

    #[error("syscall failed")]
    Syscall(#[from] SysErr),

    #[error("cross-contract call failed")]
    Call(#[from] InvokeError),

    #[error("native_exec invoke error {0}")]
    InvokeErrorCode(i32),
}

#[derive(Debug, Clone)]
pub struct Call {
    pub entrypoint: ContractEntrypointID,
    pub calldata: Calldata,
    pub gas_limit: GasAmount,
}

pub struct ExecSuccess {
    pub gas_consumed: GasAmount,
    pub return_data: Vec<u8>,
}

/// Read in a wasm from the filesystem, then deploy and call it
pub fn exec_path(wasm_path: &str, call: &Call) -> Result<ExecSuccess> {
    let wasm_bytes = std::fs::read(wasm_path)?;

    exec(&wasm_bytes, call)
}

pub type ReturnData = ();

/// Deploy the wasm contract and perform the call on it
pub fn exec(wasm_bytes: &[u8], call: &Call) -> Result<ExecSuccess> {
    let contract_blob: ContractBlob = BytesM(Vec::from(wasm_bytes));
    let contract = Contract::kWasmi {
        wasmi_contract: contract_blob,
    };

    let mut env = sdk::TestEnvironment::new();
    sdk::TestEnvironment::enable_syscall_logging();

    let contract_hash = env.setup_create_contract(&contract);
    let call = call.clone();

    let mut handler = env.prepare_test(move || -> () {
        // We'd like to somehow extract this return data ...
        let _return_data = exec_by_hash(contract_hash, call.clone()).map_err(|e|
            panic!("guest contract failed: {e}"));
    });

    // ... from this result ...
    let root_invoke_result = handler.root_invoke();
    let gas_consumed = root_invoke_result.gas_consumed;
    let invoke_result = &root_invoke_result.result;
    let invoke_error_code: i32 = invoke_result.panic;

    if invoke_error_code != sdk::InvokeErrorCode::kInvokeNone as i32 {
        return Err(Error::InvokeErrorCode(invoke_error_code));
    }

    // ... and return it here
    let return_data = vec![];
    let exec_data = ExecSuccess { gas_consumed, return_data };

    Ok(exec_data)
}

fn exec_by_hash(contract_hash: SHA256Hash, call: Call) -> Result<Vec<u8>> {

    // Deploy the contract to be tested
    let nonce = SHA256Hash::from([8; 32]);
    let deployed_addr = deploy_contract(&nonce, &contract_hash)?;

    let entrypoint = &call.entrypoint;
    let entrypoint_u64 = u64::from_be_bytes(entrypoint.into_inner());
    let mut return_len = 0;

    let result = sys::raw_Invoke(
        &deployed_addr,
        entrypoint_u64,
        call.gas_limit,
        &call.calldata,
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
