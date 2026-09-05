use counter::CounterClient;
use sdk::*;

static COUNTER_NONCE: SHA256Hash = sdk::sha256_static!("COUNTER_TEST_NONCE");

fn setup() -> (TestEnvironmentHandle, SHA256Hash) {
    let env = sdk::TestEnvironment::no_cost();
    let test_contract = counter::register_mock_contract(&env);
    let contract_hash = test_contract.as_hash().clone();
    (env, contract_hash)
}

#[test]
fn test_counter_basic() {
    let (env, contract_hash) = setup();

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(move || {
            let addr = deploy_contract(&COUNTER_NONCE, &contract_hash);

            let client = CounterClient::new(&addr);

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
    })
    .assert_success();
}

#[test]
fn test_counter_underflow() {
    let (env, contract_hash) = setup();

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(move || {
            let addr = deploy_contract(&COUNTER_NONCE, &contract_hash);

            let client = CounterClient::new(&addr);

            // Initialize with 5
            client.initialize(5);

            // Try to decrement by 10 (should abort - would go negative)
            match client.try_decrement(10) {
                Err(e) => match_exn!(e, {
                    LibraryExn::NonNegativeIntUnderflow => {}
                    _ => panic!("Unexpected error: {e:?}"),
                }),
                Ok(()) => panic!("Decrement should have failed"),
            }
        })
    })
    .assert_success();
}

#[test]
fn test_gas_limit_does_not_propagate() {
    let (env, contract_hash) = setup();

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(move || {
            let addr = deploy_contract(&COUNTER_NONCE, &contract_hash);

            let client = CounterClient::new(&addr);
            client.initialize(10);

            // Native mock instructions are unmetered, but their syscalls
            // still consume gas. A one-unit budget cannot complete get();
            // no_cost() disables storage costs, not syscall gas accounting.
            let result = client.with_gas_limit(1).try_get();
            assert!(
                matches!(&result, Err(sdk::call::RawCallError::Fault(_))),
                "gas-limited mock call should fault, got {result:?}"
            );

            // Subsequent call without explicit gas limit should use default
            // (unlimited). This verifies with_gas_limit does NOT mutate the
            // original client.
            assert_eq!(client.get(), 10);
        })
    })
    .assert_success();
}
