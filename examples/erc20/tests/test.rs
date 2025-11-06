use erc20::implementation::test_setup;
use erc20_result_test::*;

#[test]
fn check() {
    let mut env = sdk::TestEnvironment::new();

    let test_contract = test_setup::register(&mut env);
    let contract_hash = env.setup_create_contract(&test_contract);

    let mut handler = env.prepare_test(move || -> () {
        erc20_test(&contract_hash)
            .unwrap_or_else(|e| panic!("test failed: {e}"))
    });

    sdk::contract_test::grant_storage_tokens(&env, &handler);

    let res = handler.root_invoke();
    assert_eq!(res.result.panic, sdk::InvokeErrorCode::kInvokeNone as i32);

    assert!(handler.try_commit_transaction());

    let event_log = handler.get_event_logs().unwrap();
    let events = &event_log.events;

    erc20_test_event_log_check(events);
}
