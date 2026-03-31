use swap_request::*;
use sdk::*;

static CONTRACT_ADDR: ContractAddress =
    ContractAddress::new(hash_to_array!("SWAP ADDR"));

#[test]
fn test_simple() {
    sdk::client_test(|env| env.register_mock(Swap::__mock), _test_simple);
}

fn _test_simple(c: &SwapClient) {
    c.submit_request(SwapRequest::new());
    c.submit_request(SwapRequest::new());
    c.submit_request(SwapRequest::new());
    c.submit_request(SwapRequest::new());

    let last_submitted = SwapRequest::last_id();
    let last_processed = c.process_requests();

    assert_eq!(last_processed, last_submitted);
}

#[test]
fn test_multi() {
    let env = TestEnvironment::no_cost();
    let contract = env.register_mock(Swap::__mock);
    let _contract_hash = env.setup_create_contract(&contract);
    env.setup_create_deployment(&CONTRACT_ADDR, &contract);

    client_transact(&env, |c| c.submit_request(SwapRequest::new())).assert_success();
    client_transact(&env, |c| c.submit_request(SwapRequest::new())).assert_success();
    client_transact(&env, |c| c.submit_request(SwapRequest::new())).assert_success();
    env.advance_block();

    client_transact(&env, |c| {
        let last_submitted = 3; // SwapRequest::last_id();
        let last_processed = c.process_requests();

        assert_eq!(last_processed, last_submitted);
    }).assert_success();
    env.advance_block();
}

fn client_transact<F>(
    env: &TestEnvironmentHandle,
    f: F,
) -> TransactResult
where
    F: Fn(&SwapClient) + std::panic::RefUnwindSafe
{
    env.transact(|tx| {
        tx.call(&CONTRACT_ADDR).run(|| {
            let client = SwapClient::new(&CONTRACT_ADDR);
            f(&client)
        })
    })
}
