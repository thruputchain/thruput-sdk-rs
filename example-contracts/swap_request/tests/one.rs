use sdk::*;
use swap_request::*;

static CONTRACT_ADDR: ContractAddress =
    ContractAddress::new(hash_to_array!("SWAP ADDR"));

#[test]
fn test_multi() {
    let env = TestEnvironment::no_cost();
    let contract = swap_request::register_mock_contract(&env);
    contract.deploy_contract_to(&CONTRACT_ADDR);

    assert_eq!(MAX_BATCH_SIZE, 3);

    // -----------------  BLOCK ----------------------------------

    // Submit swap requests in several concurrent transactions.
    client_transact(&env, |c| c.submit_request(SwapRequest::new()))
        .assert_success();
    client_transact(&env, |c| c.submit_request(SwapRequest::new()))
        .assert_success();
    client_transact(&env, |c| c.submit_request(SwapRequest::new()))
        .assert_success();
    client_transact(&env, |c| c.submit_request(SwapRequest::new()))
        .assert_success();

    // -----------------  BLOCK ----------------------------------
    env.advance_block();

    // Use the "admin interface" to stimulate the contract to process pending
    // requests.

    client_transact(&env, |c| {
        let (n_processed, last_processed) = c.process_requests();

        assert_eq!(n_processed, 3);
        assert_eq!(last_processed, 3);
    })
    .assert_success();

    // -----------------  BLOCK ----------------------------------
    env.advance_block();

    // Process more requests

    client_transact(&env, |c| {
        let (n_processed, last_processed) = c.process_requests();

        assert_eq!(n_processed, 1);
        assert_eq!(last_processed, 4);
    })
    .assert_success();

    // -----------------  BLOCK ----------------------------------
    env.advance_block();

    // Go again, but finding no pending requests.
    client_transact(&env, |c| {
        let (n_processed, last_processed) = c.process_requests();

        assert_eq!(n_processed, 0);
        assert_eq!(last_processed, 0);
    })
    .assert_success();

    // -----------------  BLOCK ----------------------------------
    env.advance_block();

    // Submit another request ...
    client_transact(&env, |c| c.submit_request(SwapRequest::new()))
        .assert_success();

    // ... and process requests in the same block.
    client_transact(&env, |c| {
        let (n_processed, last_processed) = c.process_requests();

        assert_eq!(n_processed, 1);
        assert_eq!(last_processed, 5);
    })
    .assert_success();
}

// Helper

fn client_transact<F>(env: &TestEnvironmentHandle, f: F) -> TransactResult
where
    F: Fn(&SwapClient) + std::panic::RefUnwindSafe,
{
    static TEST_ADDR: ContractAddress =
        ContractAddress::new(hash_to_array!("TEST ADDR"));

    env.transact(|tx| {
        tx.call(&TEST_ADDR).run(|| {
            let client = SwapClient::new(&CONTRACT_ADDR);
            f(&client)
        })
    })
}
