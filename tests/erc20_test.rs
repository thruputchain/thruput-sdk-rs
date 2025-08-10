use test_exec::{Call, exec, Result};
use sdk::prelude::*;
use xdr_common::BytesM;
use xdrs::serialize_txs::encode;

#[test]
fn test() -> Result<(), Box<dyn std::error::Error + 'static>> {
    let wasm_bytes = include_bytes!("erc20.wasm");

    // Construct the call `initialize(owner = 0x)`
    let initialize_name: u64 = 0x5235C98543FF0004;
    let entrypoint = ContractEntrypointID::from_u64(initialize_name);
    let owner = ContractAddress::default();
    let calldata = BytesM(encode(&owner)?);
    let gas_limit = u64::MAX;

    let call = Call {
        entrypoint,
        calldata,
        gas_limit,
    };

    let success = exec(wasm_bytes, &call)?;

    println!("SUCCESS.  Gas consumed: {}", success.gas_consumed);

    Ok(())
}
