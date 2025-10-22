use sdk::sys::raw_LogEvent;
use sdk::contract_test_handle::handle_call;
use sdk::ContractEntrypointID;

// Simple test to demonstrate reading events that were emitted by a contract.
#[test]
fn test_event() {
    let env = sdk::TestEnvironment::new();

    let test_contract = {
        let mut contract = env.add_contract();
        let method = ContractEntrypointID::from(1u64.to_be_bytes());
        contract.register_method(method, || -> () {
            handle_call(|()| {
                log_event();
                return ();
            });
        });
        contract.as_contract()
    };

    let contract_hash = env.setup_create_contract(&test_contract);

    let mut handler = env.prepare_test(move || -> () {
        let nonce = sdk::types::SHA256Hash::from([7; 32]);
        let deployed_addr =
            sdk::deploy::deploy_contract(&nonce, &contract_hash).unwrap();

        // Invoke contract method 1
        let method =
            sdk::types::ContractEntrypointID::from(1u64.to_be_bytes());
        sdk::invoke::invoke::<(), ()>(&deployed_addr, method, u64::MAX, &())
            .unwrap_or_else(|e| panic!("contract test body failed: {}", e));
    });

    env.setup_grant_storage_tokens(
        &handler
            .get_signed_transaction()
            .tx
            .invocation
            .invoked_address,
        1_000_000_000,
    );

    let res = handler.root_invoke();
    if res.result.panic != sdk::InvokeErrorCode::kInvokeNone as i32 {
        panic!("root invocation panicked with code {}", res.result.panic);
    }

    let commit_res = handler.try_commit_transaction();
    assert!(commit_res);

    println!("event logged");

    let event_log = handler.get_event_logs().unwrap();
    let events = &event_log.events;

    assert_eq!(events.len(), 1);
    // assert_eq!(events[0].contract_address, &deployed_addr);
}

fn log_event() {
    println!("logging event");
    raw_LogEvent(None, None, None, b"test").unwrap();
}
