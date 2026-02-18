use sdk::contract_test::*;
use sdk::*;

#[test]
fn test_counter_basic() {
    let mut env = TestEnvironment::new(None);

    // Register the contract with the test environment
    let test_contract = counter::test_setup::register(&mut env);
    let contract_hash = env.setup_create_contract(&test_contract);

    contract_exec(env, move || {
        // Deploy the contract
        let nonce = SHA256Hash::from([1; 32]);
        let deployed_addr = deploy_contract(&nonce, &contract_hash).unwrap();

        // Create a client to interact with the contract
        let mut client = counter::CounterClient::new(&deployed_addr);

        // Initialize with value 10
        client.initialize(10).unwrap();

        // Check initial value
        let value = client.get().unwrap();
        assert_eq!(value, 10, "Initial value should be 10");

        // Increment by 5
        client.increment(5).unwrap();
        let value = client.get().unwrap();
        assert_eq!(value, 15, "After increment should be 15");

        // Decrement by 3
        client.decrement(3).unwrap();
        let value = client.get().unwrap();
        assert_eq!(value, 12, "After decrement should be 12");
    })
    .unwrap();
}

#[test]
fn test_counter_underflow() {
    let mut env = TestEnvironment::new(None);

    let test_contract = counter::test_setup::register(&mut env);
    let contract_hash = env.setup_create_contract(&test_contract);

    contract_exec(env, move || {
        let nonce = SHA256Hash::from([2; 32]);
        let deployed_addr = deploy_contract(&nonce, &contract_hash).unwrap();

        let mut client = counter::CounterClient::new(&deployed_addr);

        // Initialize with 5
        client.initialize(5).unwrap();

        // Try to decrement by 10 (should fail - would go negative)
        let result = client.decrement(10);

        // Should get an abort error
        match result {
            Err(InvokeError::AbortedWith(_)) => {
                // Expected - counter refused to go negative
            }
            Ok(_) => panic!("Decrement should have failed"),
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    })
    .unwrap();
}
