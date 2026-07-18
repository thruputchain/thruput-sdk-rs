use sdk::*;

#[test]
fn velocity_limit_smoke_test() {
    let env = sdk::TestEnvironment::no_cost();

    let velocity_limit_mock = velocity_limit::register_mock_contract(&env);
    let token_mock = erc20::register_mock_contract(&env);

    let velocity_limit_addr: ContractAddress = [0xAA; 32].into();
    let token_addr: ContractAddress = [0xBB; 32].into();

    velocity_limit_mock.deploy_contract_to(&velocity_limit_addr);
    token_mock.deploy_contract_to(&token_addr);

    let owner = ContractAddress::named("owner");
    let spender = ContractAddress::named("spender");

    env.transact(|tx| {
        tx.call(&owner).run(move || {
            // initialize the erc20 contract, minting tokens to the velocity limit contract
            let erc20 = groundhog_erc20_if::ERC20Client::new(&token_addr);
            erc20.initialize(&owner);

            erc20.mint(&velocity_limit_addr, 1000000000000000000);

            // initialize the velocity limit contract
            let velocity_limit = velocity_limit::VelocityLimitedWithdrawalClient::new(&velocity_limit_addr);
            velocity_limit.initialize(owner, token_addr);

            // create a velocity limit for the spender
            velocity_limit.set_limit(spender, 100, /* frequency */ 2.into());
        })
    })
    .assert_success();

    env.advance_block();

    std::println!("Advanced block");

    // withdraw the full limit in one tx
    env.transact(|tx| {
        tx.call(&spender).run(move || {
            let velocity_limit = velocity_limit::VelocityLimitedWithdrawalClient::new(&velocity_limit_addr);
            velocity_limit.withdraw(100);
        })
    })
    .assert_success();

    std::println!("Withdrew full limit");

    // a concurrent tx cannot withdraw anything
    env.transact(|tx| {
        tx.call(&spender).run(move || {
            let velocity_limit = velocity_limit::VelocityLimitedWithdrawalClient::new(&velocity_limit_addr);
            velocity_limit.withdraw(1);
        })
    })
    .assert_commit_failed();

    env.advance_block();

    // limit is not reset yet
    env.transact(|tx| {
        tx.call(&spender).run(move || {
            let velocity_limit = velocity_limit::VelocityLimitedWithdrawalClient::new(&velocity_limit_addr);
            velocity_limit.withdraw(1);
        })
    })
    .assert_commit_failed();

    env.advance_block();

    // limit is reset, so the tx succeeds
    env.transact(|tx| {
        tx.call(&spender).run(move || {
            let velocity_limit = velocity_limit::VelocityLimitedWithdrawalClient::new(&velocity_limit_addr);
            velocity_limit.withdraw(10);
        })
    })
    .assert_success();
}
