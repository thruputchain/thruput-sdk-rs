use sdk::*;

/// Deploy the velocity-limit and token contracts, mint tokens to the
/// velocity-limit contract, and give `spender` a limit of 100 per 2 blocks.
/// Returns the velocity-limit contract address and the spender.
fn setup(env: &TestEnvironmentHandle) -> (ContractAddress, ContractAddress) {
    let velocity_limit_mock = velocity_limit::register_mock_contract(env);
    let token_mock = erc20::register_mock_contract(env);

    let velocity_limit_addr: ContractAddress = [0xAA; 32].into();
    let token_addr: ContractAddress = [0xBB; 32].into();

    velocity_limit_mock.deploy_contract_to(&velocity_limit_addr);
    token_mock.deploy_contract_to(&token_addr);

    let owner = ContractAddress::named("owner");
    let spender = ContractAddress::named("spender");

    env.transact(|tx| {
        tx.call(&owner).run(move || {
            // initialize the erc20 contract, minting tokens to the velocity
            // limit contract
            let erc20 = groundhog_erc20_if::ERC20Client::new(&token_addr);
            erc20.initialize(&owner);
            erc20.mint(&velocity_limit_addr, 1000000000000000000);

            // initialize the velocity limit contract
            let velocity_limit =
                velocity_limit::VelocityLimitedWithdrawalClient::new(
                    &velocity_limit_addr,
                );
            velocity_limit.initialize(owner, token_addr);

            // create a velocity limit for the spender
            velocity_limit.set_limit(
                spender,
                100,
                /* frequency */ 2.into(),
            );
        })
    })
    .assert_success();

    env.advance_block();

    (velocity_limit_addr, spender)
}

#[test]
fn velocity_limit_smoke_test() {
    let env = sdk::TestEnvironment::no_cost();
    let (velocity_limit_addr, spender) = setup(&env);

    // withdraw the full limit in one tx
    env.transact(|tx| {
        tx.call(&spender).run(move || {
            let velocity_limit =
                velocity_limit::VelocityLimitedWithdrawalClient::new(
                    &velocity_limit_addr,
                );
            velocity_limit.withdraw(100);
        })
    })
    .assert_success();

    // a concurrent tx cannot withdraw anything
    env.transact(|tx| {
        tx.call(&spender).run(move || {
            let velocity_limit =
                velocity_limit::VelocityLimitedWithdrawalClient::new(
                    &velocity_limit_addr,
                );
            velocity_limit.withdraw(1);
        })
    })
    .assert_commit_failed();

    env.advance_block();

    // limit is not reset yet
    env.transact(|tx| {
        tx.call(&spender).run(move || {
            let velocity_limit =
                velocity_limit::VelocityLimitedWithdrawalClient::new(
                    &velocity_limit_addr,
                );
            velocity_limit.withdraw(1);
        })
    })
    .assert_commit_failed();

    env.advance_block();

    // limit is reset, so the tx succeeds
    env.transact(|tx| {
        tx.call(&spender).run(move || {
            let velocity_limit =
                velocity_limit::VelocityLimitedWithdrawalClient::new(
                    &velocity_limit_addr,
                );
            velocity_limit.withdraw(10);
        })
    })
    .assert_success();
}

#[test]
fn withdrawals_accumulate_within_a_transaction() {
    let env = sdk::TestEnvironment::no_cost();
    let (velocity_limit_addr, spender) = setup(&env);

    // Two withdrawals in one transaction accumulate: 60 + 60 exceeds the
    // limit of 100, so the transaction cannot commit.
    env.transact(|tx| {
        tx.call(&spender).run(move || {
            let velocity_limit =
                velocity_limit::VelocityLimitedWithdrawalClient::new(
                    &velocity_limit_addr,
                );
            velocity_limit.withdraw(60);
            velocity_limit.withdraw(60);
        })
    })
    .assert_commit_failed();

    // The failed transaction consumed nothing; 60 + 30 fits.
    env.transact(|tx| {
        tx.call(&spender).run(move || {
            let velocity_limit =
                velocity_limit::VelocityLimitedWithdrawalClient::new(
                    &velocity_limit_addr,
                );
            velocity_limit.withdraw(60);
            velocity_limit.withdraw(30);
        })
    })
    .assert_success();

    // Consumption also accumulates across transactions in the same block:
    // 90 + 20 exceeds the limit.
    env.transact(|tx| {
        tx.call(&spender).run(move || {
            let velocity_limit =
                velocity_limit::VelocityLimitedWithdrawalClient::new(
                    &velocity_limit_addr,
                );
            velocity_limit.withdraw(20);
        })
    })
    .assert_commit_failed();
}
