use caller::{CallerClient, CallerError};
use groundhog_erc20_if::ERC20Client;
use groundhog_erc20_if::ERC20Error;
use sdk::*;
use thiserror::Error;

// Load two contracts into the same test environment, and
// have one call the other.
#[test]
fn caller_test() {
    let mut env = sdk::TestEnvironment::new();

    let erc20_contract = erc20::test_setup::register(&mut env);
    let erc20_hash = env.setup_create_contract(&erc20_contract);

    let caller_contract = caller::test_setup::register(&mut env);
    let caller_hash = env.setup_create_contract(&caller_contract);

    let mut handler = env.prepare_test(move || -> () {
        caller_test_inner(&erc20_hash, &caller_hash)
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
    contract_hash: &SHA256Hash,
) -> Result<(), TestError> {
    let erc20_nonce = SHA256Hash::from([6; 32]);
    let erc20_addr = deploy_contract(&erc20_nonce, erc20_hash)?;
    let erc20_owner = get_current_address()?;

    let mut erc20 = ERC20Client::new(&erc20_addr);
    erc20.initialize(&erc20_owner)?;

    assert_eq!(0, erc20.total_supply()?);
    assert_eq!(0, erc20.get_balance()?);

    let nonce = SHA256Hash::from([7; 32]);
    let deployed_addr = deploy_contract(&nonce, contract_hash)?;

    let mut caller = CallerClient::new(&deployed_addr);
    caller.initialize(&erc20_addr)?;

    let some_account = ContractAddress::zero();
    assert_eq!(0, caller.check_balance(&some_account)??);

    Ok(())
}
