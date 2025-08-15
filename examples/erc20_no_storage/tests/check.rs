use erc20_no_storage::test_setup;
use erc20_no_storage::ERC20Client;
use erc20_no_storage::interface::Error as ERC20Error;
use sdk::*;
use thiserror::Error;

#[test]
fn check() {
    let mut env = sdk::TestEnvironment::new();

    let test_contract = test_setup::register(&mut env);
    let contract_hash = env.setup_create_contract(&test_contract);
    // sdk::TestEnvironment::enable_syscall_logging();

    let mut handler = env.prepare_test(move || -> () {
        check_inner(&contract_hash).unwrap_or_else(|e|
            panic!("test failed: {e}"))
    });

    let res = handler.root_invoke();
    assert_eq!(res.result.panic, sdk::InvokeErrorCode::kInvokeNone as i32);
}

#[derive(Error, Debug, Clone)]
enum TestError {
    #[error("contract error: {0}")]
    Contract(#[from] ERC20Error),

    #[error("client error: {0}")]
    Client(#[from] ClientError<ERC20Error>),

    #[error("system error: {0}")]
    Sys(#[from] SysErr),
}

fn check_inner(contract_hash: &sdk::types::SHA256Hash) -> Result<(), TestError> {
    let nonce: SHA256Hash = [7; 32].into();
    let deployed_addr = deploy_contract(&nonce, contract_hash)?;
    let mut client = ERC20Client::new(&deployed_addr);

    let owner = get_current_address()?;
    client.initialize(&owner)?;

    assert_eq!(client.total_supply()?, 0);
    assert_eq!(client.get_balance()?, 0);

    client.mint(&owner, 100)?;
    assert_eq!(client.get_balance()?, 100);

    let other: ContractAddress = [0xBB; 32].into();
    client.transfer(&other, 25)?;
    assert_eq!(client.get_balance_of(&other)?, 25);
    assert_eq!(client.get_balance_of(&owner)?, 75);

    client.burn(&other, 5)?;
    assert_eq!(client.get_balance_of(&other)?, 20);

    assert_eq!(client.total_supply()?, 95);

    Ok(())
}
