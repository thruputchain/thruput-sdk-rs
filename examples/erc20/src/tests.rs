use crate::implementation;
use crate::interface::Result;
use crate::interface::client::ERC20Client;
use sdk::prelude::*;

#[test]
fn test_erc20() {
    let mut env = sdk::TestEnvironment::new();

    let test_contract = implementation::test_setup::register(&mut env);
    let contract_hash = env.setup_create_contract(&test_contract);
    // sdk::TestEnvironment::enable_syscall_logging();

    let mut handler = env.prepare_test(move || -> () {
        match test_erc20_inner(&contract_hash) {
            Ok(_) => (),
            Err(error) => panic!("test failed: {}", error),
        }
    });

    let res = handler.root_invoke();
    assert_eq!(res.result.panic, sdk::InvokeErrorCode::kInvokeNone as i32);
}

fn test_erc20_inner(contract_hash: &sdk::types::SHA256Hash) -> Result<()> {
    let nonce: SHA256Hash = [7; 32].into();
    let deployed_addr = deploy_contract(&nonce, contract_hash)?;
    let owner = get_current_address()?;

    let mut client = ERC20Client::new(&deployed_addr);
    client.initialize(&owner)?;

    assert_eq!(client.total_supply().unwrap(), 0);
    assert_eq!(client.get_balance().unwrap(), 0);

    client.mint(&owner.clone().into(), 100).unwrap();
    assert_eq!(client.get_balance().unwrap(), 100);

    let other: ContractAddress = [0xBB; 32].into();
    client.transfer(&other, 25)?;
    assert_eq!(client.get_balance_of(&other)?, 25);
    assert_eq!(client.get_balance_of(&owner)?, 75);

    client.burn(&other, 5).unwrap();
    assert_eq!(client.get_balance_of(&other).unwrap(), 20);

    assert_eq!(client.total_supply().unwrap(), 95);

    Ok(())
}
