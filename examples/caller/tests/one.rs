use erc20_if as interface;
use interface::ERC20;
use interface::Result as ERC20Result;
use interface::client::ERC20Client;
use caller::interface::ICaller;
use caller::interface::client::ICallerClient;
use sdk::prelude::*;

#[test]
fn caller_test() {
    let mut env = sdk::TestEnvironment::new();

    let erc20_contract = erc20::test_setup::register(&mut env);
    let erc20_hash = env.setup_create_contract(&erc20_contract);

    let test_contract = caller::test_setup::register(&mut env);
    let contract_hash = env.setup_create_contract(&test_contract);

    let mut handler = env.prepare_test(move || -> () {
        match caller_test_inner(&erc20_hash, &contract_hash) {
            Ok(_) => (),
            Err(error) => panic!("test failed: {}", error),
        }
    });

    let res = handler.root_invoke();
    assert_eq!(res.result.panic, sdk::InvokeErrorCode::kInvokeNone as i32);
}

fn caller_test_inner(erc20_hash: &SHA256Hash, contract_hash: &SHA256Hash) -> Result<()> {
    let erc20_nonce = SHA256Hash::from([6; 32]);
    let erc20_addr = deploy_contract(&erc20_nonce, erc20_hash)?;
    let erc20_owner = get_current_address().unwrap();

    let mut erc20 = ERC20Client::new(&erc20_addr);
    assert_eq!(
        ERC20::initialize(&mut erc20, &erc20_owner),
        ERC20Result::Ok(()),
    );

    assert_eq!(erc20.total_supply(), ERC20Result::Ok(0));
    assert_eq!(erc20.get_balance(), ERC20Result::Ok(0));

    let nonce = SHA256Hash::from([7; 32]);
    let deployed_addr = deploy_contract(&nonce, contract_hash)?;

    let mut caller = ICallerClient::new(&deployed_addr);
    ICaller::initialize(&mut caller, &erc20_addr);

    let some_account = ContractAddress::zero();
    assert_eq!(caller.check_balance(&some_account), 0);

    Ok(())
}
