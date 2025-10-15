//! Test one contract calling the other, both running in wasm

use caller::{CallerClient, CallerError};
use groundhog_erc20_if::ERC20Client;
use groundhog_erc20_if::ERC20Error;
use sdk::contract_test::load_wasm_contract;
use sdk::*;
use thiserror::Error;

// test working directory is example contract directory
const ERC20_PATH: &str = "../../target/optimized/erc20.wasm";
const CALLER_PATH: &str = "../../target/optimized/caller.wasm";

#[test]
fn caller_test() {
    let erc20_contract = load_wasm_contract(ERC20_PATH);
    let caller_contract = load_wasm_contract(CALLER_PATH);

    let env = sdk::TestEnvironment::new();

    let erc20_hash = env.setup_create_contract(&erc20_contract);
    let contract_hash = env.setup_create_contract(&caller_contract);

    let mut handler = env.prepare_test(move || -> () {
        caller_test_inner(&erc20_hash, &contract_hash)
            .unwrap_or_else(|e| panic!("test failed: {e}"))
    });

    let res = handler.root_invoke();
    assert_eq!(res.result.panic, sdk::InvokeErrorCode::kInvokeNone as i32);
}

#[derive(Error, Debug, Clone)]
enum TestError {
    #[error("caller error: {0}")]
    Caller(#[from] CallerError),

    #[error("ERC20 error: {0}")]
    ERC20(#[from] ERC20Error),

    #[error("client error: {0}")]
    Client(#[from] ClientError<ERC20Error>),

    #[error("invoke error: {0}")]
    Invoke(#[from] InvokeError),

    #[error("system error: {0}")]
    Sys(#[from] SysErr),
}

fn caller_test_inner(
    erc20_hash: &SHA256Hash,
    caller_hash: &SHA256Hash,
) -> Result<(), TestError> {
    let erc20_nonce = SHA256Hash::from([6; 32]);
    let erc20_addr = deploy_contract(&erc20_nonce, erc20_hash)?;
    let erc20_owner = get_current_address()?;

    let mut erc20 = ERC20Client::new(&erc20_addr);
    erc20.initialize(&erc20_owner)?;

    assert_eq!(0, erc20.total_supply()?);
    assert_eq!(0, erc20.get_balance()?);

    let caller_nonce = SHA256Hash::from([7; 32]);
    let deployed_addr = deploy_contract(&caller_nonce, caller_hash)?;

    let mut caller = CallerClient::new(&deployed_addr);
    caller.initialize(&erc20_addr)?;

    // Tell the caller to call the erc20 contract
    let some_account = ContractAddress::zero();
    assert_eq!(0, caller.check_balance(&some_account)??);

    println!("GOT BALANCE");

    Ok(())
}
