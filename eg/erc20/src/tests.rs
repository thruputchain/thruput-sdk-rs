use crate::implementation;
use crate::interface::client::ERC20Client;
use crate::interface::ERC20;
use sdk::result::Result;

#[test]
fn check_erc20() {
    let mut env = sdk::TestEnvironment::new();

    let test_contract = implementation::test_setup::register(&mut env);
    let contract_hash = env.setup_create_contract(&test_contract);

    let mut handler = env.prepare_test(move || -> i32 {
        match check_erc20_inner(&contract_hash) {
            Ok(_) => 0,
            Err(error) => panic!("test failed: {}", error),
        }
    });

    let res = handler.root_invoke();
    assert_eq!(res.result.panic, sdk::InvokeErrorCode::kNone as i32);
}

fn check_erc20_inner(contract_hash: &sdk::types::SHA256Hash) -> Result<()> {
    use sdk::prelude::*;

    let nonce = ByteArray::new([7; 32]);
    let deployed_addr = deploy_contract(&nonce, contract_hash).unwrap();
    let owner = get_current_address().unwrap();

    let mut client = ERC20Client::new(&deployed_addr);
    ERC20Client::initialize(Some(&mut client), &owner)?;

    assert_eq!(client.total_supply()?, 0);
    assert_eq!(client.get_balance()?, 0);

    client.mint(&owner, 100)?;
    assert_eq!(client.get_balance()?, 100);

    let other = ByteArray::new([0xBB; 32]);
    client.transfer(&other, 25)?;
    assert_eq!(client.get_balance_of(&other)?, 25);
    assert_eq!(client.get_balance_of(&owner)?, 75);

    client.burn(&other, 5)?;
    assert_eq!(client.get_balance_of(&other)?, 20);

    assert_eq!(client.total_supply()?, 95);

    Ok(())
}
