pub use erc20_test::*;

use sdk::TestEnvironmentHandle;
use sdk::contract_test::deploy_address;
use sdk::types::Contract;

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

    let mut handler = {
        let model_handle = model.clone();

        env.prepare_test(move || -> () {
            let deployed_addr = erc20_test_setup(&test_contract_hash).unwrap();

            let model = erc20_test(&deployed_addr)
                .unwrap_or_else(|e| panic!("test failed: {e}"));

            *model_handle.lock().unwrap() = Some(model);
        })
    };

    let res = handler.root_invoke();
    assert_eq!(res.result.panic, sdk::InvokeErrorCode::kInvokeNone as i32);

    assert!(handler.try_commit_transaction());

    let event_log = handler.get_event_logs();

    // Print out the transaction's event log
    {
        use groundhog_erc20_if::*;
        use sdk::EmittedEvent;

        if let Some(ref log) = event_log {
            for (i, event) in log.events.iter().enumerate() {
                let event: EmittedEvent = event.into();
                let event: ERC20Event = event.try_into().unwrap();
                eprintln!("\tevent {:?}: {}", i, event);
            }
        }
    }

    // Check that the event log matches the model
    {
        let m = model.lock().unwrap().clone().unwrap();

        if let Some(ref log) = event_log {
            m.check_events(&log.events);
        } else {
            let no_events = [];
            m.check_events(&no_events);
        }
    }
}
