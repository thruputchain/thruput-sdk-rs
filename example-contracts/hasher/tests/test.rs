use hasher::{test_setup, HasherClient};

pub static TEST_NONCE: sdk::SHA256Hash = sdk::sha256_static!("EXCEPTIONS_TEST_NONCE");

#[test]
fn hasher_test() {
    let env = sdk::TestEnvironment::new(Some(&sdk::StorageCostModel::no_cost_model()));

    // Register the contract with the test environment
    let test_contract = test_setup::register(&mut env);
    let contract_hash = env.setup_create_contract(&test_contract);

    sdk::contract_test_invoke(env, move || {
        // Deploy the contract under test
        let addr = sdk::deploy_contract(&TEST_NONCE, &contract_hash);

        // Create a client to interact with the contract
        let mut hasher = HasherClient::new(&addr);

        hasher.run();

    })
    .unwrap_or_else(|e|
        panic!("Test invocation failed: {}", e));
}
