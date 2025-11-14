use groundhog_erc20_if::*;
pub use erc20_test::*;

use sdk::TestEnvironmentHandle;
use sdk::contract_test::deploy_address;
use sdk::types::Contract;
use sdk::types::event::TxEmittedEvent;

pub fn test_erc20_contract(
    env: &TestEnvironmentHandle,
    test_contract: &Contract,
) {
    let test_contract_hash = env.setup_create_contract(test_contract);
    let test_contract_addr =
        deploy_address(&OWNER, &NONCE, &test_contract_hash);

    env.setup_grant_storage_tokens(&test_contract_addr, 1_000_000_000);

    // It's currently difficult to extract state from mock contracts,
    // so we'll allocate shared storage for the model we want to extract
    // after the test run:
    use std::sync::{Arc, Mutex};
    let model: Arc<Mutex<Option<Model>>> = Arc::new(Mutex::new(None));
    let model2 = model.clone();

    let mut handler = env.prepare_test(move || -> () {
        let deployed_addr = erc20_test_setup(&test_contract_hash).unwrap();

        let model = erc20_test(&deployed_addr)
            .unwrap_or_else(|e| panic!("test failed: {e}"));

        *model2.lock().unwrap() = Some(model);
    });

    let res = handler.root_invoke();
    assert_eq!(res.result.panic, sdk::InvokeErrorCode::kInvokeNone as i32);

    assert!(handler.try_commit_transaction());

    let event_log = handler.get_event_logs().unwrap();
    let events: &[TxEmittedEvent] = event_log.events.as_ref();

    // Print out the transaction's event log
    for (i, event) in events.iter().enumerate() {
        let event: ERC20Event = event.try_into().unwrap();
        eprintln!("\tevent {:?}: {}", i, event);
    }

    let m = model.lock().unwrap().clone().unwrap();

    // Check that the event log matches the model
    m.check_events(events);
}
