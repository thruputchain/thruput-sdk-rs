use counter::{CounterClient, CounterClientError};
use sdk::*;

static COUNTER_NONCE: SHA256Hash = sdk::sha256_static!("COUNTER_TEST_NONCE");

#[test]
fn test_counter_basic() {
    let env = sdk::TestEnvironment::no_cost();

    let test_contract = env.register_mock(counter::Counter::__mock);
    let contract_hash = env.setup_create_contract(&test_contract);

    contract_exec(env, move || {
        let addr = deploy_contract(&COUNTER_NONCE, &contract_hash);

        let mut client = CounterClient::new(&addr);

        // Initialize with value 10
        client.initialize(10);

        // Check initial value
        assert_eq!(client.get(), 10);

        // Increment by 5
        client.increment(5);
        assert_eq!(client.get(), 15);

        // Decrement by 3
        client.decrement(3);
        assert_eq!(client.get(), 12);
    })
    .unwrap_or_else(|e| panic!("Test failed: {e:?}"));
}

#[test]
fn test_counter_underflow() {
    let env = sdk::TestEnvironment::no_cost();

    let test_contract = env.register_mock(counter::Counter::__mock);
    let contract_hash = env.setup_create_contract(&test_contract);

    contract_exec(env, move || {
        let addr = deploy_contract(&COUNTER_NONCE, &contract_hash);

        let mut client = CounterClient::new(&addr);

        // Initialize with 5
        client.initialize(5);

        // Try to decrement by 10 (should abort - would go negative)
        match client.try_decrement(10) {
            Err(CounterClientError::Library(ref lib_exn)) => {
                assert!(
                    matches!(lib_exn, LibraryExn::NonNegativeIntUnderflow),
                    "Expected NonNegativeIntUnderflow, got: {:?}",
                    lib_exn
                );
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
            Ok(()) => panic!("Decrement should have failed"),
        }
    })
    .unwrap_or_else(|e| panic!("Test failed: {e:?}"));
}
