mod model;
mod erc20_test;

use erc20_test::*;
use sdk::types::Contract;
use sdk::TestEnvironmentHandle;
use sdk::contract_test::deploy_address;

pub fn test_erc20_contract(env: &TestEnvironmentHandle, test_contract: &Contract) {
    let test_contract_hash = env.setup_create_contract(test_contract);
    let test_contract_addr = deploy_address(&OWNER, &NONCE, &test_contract_hash);

    env.setup_grant_storage_tokens(&test_contract_addr, 1_000_000_000);

    let mut handler = env.prepare_test(move || -> () {
        erc20_test(&test_contract_hash)
            .unwrap_or_else(|e| panic!("test failed: {e}"))
    });

    let res = handler.root_invoke();
    assert_eq!(res.result.panic, sdk::InvokeErrorCode::kInvokeNone as i32);

    assert!(handler.try_commit_transaction());

    let event_log = handler.get_event_logs().unwrap();

    erc20_test_event_log_check(&event_log.events);
}
