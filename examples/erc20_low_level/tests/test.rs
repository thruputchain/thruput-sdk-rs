use erc20_low_level::test;
use erc20_low_level::ERC20Client;
use erc20_low_level::interface::Error as ERC20Error;
use sdk::prelude::*;

#[test]
fn erc20_ll_check() {
    let mut env = sdk::TestEnvironment::new();

    let test_contract = test::register(&mut env);
    let contract_hash = env.setup_create_contract(&test_contract);
    // sdk::TestEnvironment::enable_syscall_logging();

    let mut handler = env.prepare_test(move || -> () {
        let nonce: SHA256Hash = [7; 32].into();
        let deployed_addr = deploy_contract(&nonce, &contract_hash).unwrap();
        let mut client = ERC20Client::new(&deployed_addr);

        check_inner(&mut client).unwrap_or_else(|e|
            panic!("test failed: {e}"))
    });

    let res = handler.root_invoke();
    assert_eq!(res.result.panic, sdk::InvokeErrorCode::kInvokeNone as i32);
}

fn check_inner(client: &mut ERC20Client) -> ClientResult<(), ERC20Error> {
    let owner = get_current_address().unwrap();
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
