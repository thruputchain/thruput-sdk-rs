use thruput_sdk::*;

#[test]
fn signer_pass_back_and_forth() {
    let env = thruput_sdk::TestEnvironment::no_cost();

    let callee_mock = signer_callee::register_mock_contract(&env);
    let caller_mock = signer_caller::register_mock_contract(&env);

    let callee_addr: ContractAddress = [0xCC; 32].into();
    let caller_addr: ContractAddress = [0xBB; 32].into();

    callee_mock.deploy_contract_to(&callee_addr);
    caller_mock.deploy_contract_to(&caller_addr);

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(move || {
            let caller = signer_caller::SignerCallerClient::new(&caller_addr);

            std::println!("got here");

            let token = caller.mint_and_forward(&callee_addr, 100);
            std::println!("got here 2");
            caller.consume_minted_token(token, 100);
            std::println!("got here 3");
        })
    })
    .assert_success();
}
