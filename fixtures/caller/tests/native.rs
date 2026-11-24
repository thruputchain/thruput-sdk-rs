mod common;
use caller::CallerClient;
use common::*;
use thruput_erc20_if::ERC20Client;
use thruput_sdk::{CodeHash, Named};

// Load two contracts into the same test environment, and
// have one call the other.
#[test]
fn caller_test_native() {
    let env = thruput_sdk::TestEnvironment::no_cost();

    let erc20_contract = erc20::register_mock_contract(&env);
    let erc20_code = CodeHash::<ERC20Client>::of_mock(&erc20_contract);

    let caller_contract = caller::register_mock_contract(&env);
    let caller_code = CodeHash::<CallerClient>::of_mock(&caller_contract);

    env.transact(|tx| {
        tx.call(&thruput_sdk::ContractAddress::named("test"))
            .run(move || {
                caller_test_inner(&erc20_code, &caller_code);
            })
    })
    .assert_success();
}
