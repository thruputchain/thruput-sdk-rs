use crate::implementation;
use crate::interface::client::ERC20Client;
use crate::interface::ERC20;
use sdk::result::Result;

use std::println;

#[test]
fn check_mint() {
    let mut env = sdk::test_environment::TestEnvironment::new();

    let test_contract = implementation::test_setup::register(&mut env);
    let contract_hash = env.setup_create_contract(&test_contract);
    sdk::test_environment::TestEnvironment::enable_syscall_logging();

    let mut handler = env.prepare_test(move || -> i32 {
        match check_mint_inner(&contract_hash) {
            Ok(_) => 0,
            Err(error) => panic!("test failed: {}", error),
        }
    });

    let res = handler.root_invoke();
    assert_eq!(res.result.panic, sdk::InvokeErrorCode::kInvokeNone as i32);
}

fn check_mint_inner(contract_hash: &sdk::types::SHA256Hash) -> Result<()> {
    use sdk::prelude::*;

    let nonce : sdk::ByteArray<32> = [7; 32].into();
    let deployed_addr = deploy_contract(&nonce.into(), contract_hash).unwrap();
    let owner = get_current_address().unwrap();

    let mut client = ERC20Client::new(&deployed_addr);
    ERC20Client::initialize(Some(&mut client), &owner).unwrap();

    assert_eq!(client.total_supply().unwrap(), 0);
    assert_eq!(client.get_balance().unwrap(), 0);

    client.mint(&owner.clone().into(), 100).unwrap();
    assert_eq!(client.get_balance().unwrap(), 100);

    let other : sdk::ByteArray<32> = [0xBB; 32].into();
    client.transfer(&other.into(), 25).unwrap();
    assert_eq!(client.get_balance_of(&other.into()).unwrap(), 25);
    assert_eq!(client.get_balance_of(&owner.into()).unwrap(), 75);

    client.burn(&other.into(), 5).unwrap();
    assert_eq!(client.get_balance_of(&other.into()).unwrap(), 20);

    assert_eq!(client.total_supply().unwrap(), 95);

    Ok(())
}
