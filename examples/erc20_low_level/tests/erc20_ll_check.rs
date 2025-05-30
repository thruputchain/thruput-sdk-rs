use erc20_low_level::test;
use erc20_low_level::interface::client::ERC20Client;
use erc20_low_level::interface::ERC20;
use erc20_low_level::interface::Result as ERC20Result;
use sdk::prelude::*;

#[test]
fn erc20_ll_check() {
    let mut env = sdk::TestEnvironment::new();

    let test_contract = test::register(&mut env);
    let contract_hash = env.setup_create_contract(&test_contract);
    sdk::TestEnvironment::enable_syscall_logging();

    let mut handler = env.prepare_test(move || -> () {
        match check_erc20_inner(&contract_hash) {
            Ok(_) => (),
            Err(error) => panic!("test failed: {}", error),
        }
    });

    let res = handler.root_invoke();
    assert_eq!(res.result.panic, sdk::InvokeErrorCode::kInvokeNone as i32);
}

fn check_erc20_inner(contract_hash: &sdk::types::SHA256Hash) -> ERC20Result<()> {
    let nonce: SHA256Hash = [7; 32].into();
    let deployed_addr = deploy_contract(&nonce, contract_hash)?;
    let owner = get_current_address().unwrap();

    let mut client = ERC20Client::new(&deployed_addr);
    ERC20Client::initialize(Some(&mut client), &owner)?;

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
