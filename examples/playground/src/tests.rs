use crate::implementation;
use crate::ERC20Client;
use sdk::prelude::*;

#[test]
fn check_playground() {
    let mut env = sdk::TestEnvironment::new();

    let test_contract = implementation::test_setup::register(&mut env);
    let contract_hash = env.setup_create_contract(&test_contract);

    let mut handler = env.prepare_test(move || -> () {
        match check_playground_inner(&contract_hash) {
            Ok(_) => (),
            Err(error) => panic!("test failed: {}", error),
        }
    });

    let res = handler.root_invoke();
    assert_eq!(res.result.panic, sdk::InvokeErrorCode::kInvokeNone as i32);
}

fn check_playground_inner(
    contract_hash: &sdk::types::SHA256Hash,
) -> Result<()> {
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
