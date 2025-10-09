use erc20::implementation;
use erc20::ERC20Client;
use sdk::*;

#[test]
fn test_erc20() {
    let mut env = sdk::TestEnvironment::new();

    let test_contract = implementation::test_setup::register(&mut env);
    let contract_hash = env.setup_create_contract(&test_contract);

    let mut handler = env.prepare_test(move || -> () {
        let nonce: SHA256Hash = [7; 32].into();
        let deployed_addr = deploy_contract(&nonce, &contract_hash).unwrap();
        let mut client = ERC20Client::new(&deployed_addr);

        test_inner(&mut client);
    });

    let res = handler.root_invoke();
    if res.result.panic != sdk::InvokeErrorCode::kInvokeNone as i32 {
        panic!("root invocation panicked with code {}", res.result.panic);
    }
}

fn test_inner(client: &mut ERC20Client) {
    let owner = get_current_address().unwrap();
    client.initialize(&owner).unwrap_or_else(|e|
        panic!("Contract-call failed: {e}"));

    assert_eq!(client.total_supply().unwrap(), 0);

    let bal = client.get_balance().unwrap_or_else(|e|
        panic!("get_balance failed: {e}"));

    assert_eq!(bal, 0);

    client.mint(&owner.clone().into(), 100).unwrap();
    assert_eq!(client.get_balance().unwrap(), 100);

    let other: ContractAddress = [0xBB; 32].into();
    client.transfer(&other, 25).unwrap();
    assert_eq!(client.get_balance_of(&other).unwrap(), 25);
    assert_eq!(client.get_balance_of(&owner).unwrap(), 75);

    client.burn(&other, 5).unwrap();
    assert_eq!(client.get_balance_of(&other).unwrap(), 20);

    assert_eq!(client.total_supply().unwrap(), 95);
}
