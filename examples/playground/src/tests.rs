use crate::implementation;
use crate::ERC20Client;
use sdk::contract_result::Error;
use sdk::prelude::*;
use thiserror::Error;

#[test]
fn check_playground() {
    let mut env = sdk::TestEnvironment::new();

    let test_contract = implementation::test_setup::register(&mut env);
    let contract_hash = env.setup_create_contract(&test_contract);

    let mut handler = env.prepare_test(move || -> () {
        check_inner(&contract_hash).unwrap_or_else(|e| {
            panic!("test failed: {e}");
        })
    });

    let res = handler.root_invoke();
    assert_eq!(res.result.panic, sdk::InvokeErrorCode::kInvokeNone as i32);
}

#[derive(Error, Debug, Clone)]
enum TestError {
    #[error("contract error: {0}")]
    Contract(#[from] Error),

    #[error("client error: {0}")]
    Client(#[from] ClientError<Error>),

    #[error("invoke error: {0}")]
    Invoke(#[from] InvokeError),

    #[error("system error: {0}")]
    Sys(#[from] SysErr),
}

fn check_inner(contract_hash: &sdk::types::SHA256Hash) -> Result<(), TestError> {
    let nonce: SHA256Hash = [7; 32].into();
    let deployed_addr = deploy_contract(&nonce, contract_hash)?;
    let owner = get_current_address().unwrap();

    let mut client = ERC20Client::new(&deployed_addr);
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

    client.add_frot(10)?;
    let f = client.get_frot()?;
    assert_eq!(f, 10, "Frot is not");

    let buf = [0u8, 1, 2];
    let len_reported = client.hello(&buf[..]);
    assert_eq!(len_reported, Ok(buf.len() as u32));

    Ok(())
}
