pub use erc20_test::*;

use sdk::TestEnvironmentHandle;
use sdk::types::Contract;

pub fn test_erc20_contract(
    env: TestEnvironmentHandle,
    test_contract: &Contract,
) {
    let test_contract_hash = env.setup_create_contract(test_contract);

    // It's currently difficult to extract state from mock contracts,
    // so we'll allocate shared storage for the model we want to extract
    // after the test run:
    use std::sync::{Arc, Mutex};
    let model: Arc<Mutex<Option<Model>>> = Arc::new(Mutex::new(None));

    let mut handler = {
        let model_handle = model.clone();

        sdk::contract_test_handler(env, move || {
            let deployed_addr = erc20_test_setup(&test_contract_hash);

            let model = erc20_test(&deployed_addr);

            *model_handle.lock().unwrap() = Some(model);
        })
    };

    let res = handler.root_invoke();
    if res.output.status_code()
        != sdk::types::transaction::InvocationFrameStatusCode::Return
    {
        panic!(
            "ERC20 test invocation failed with code {}",
            res.output.status_code()
        );
    }

    match handler.try_commit_transaction() {
        sdk::ExecResult::ExecResultOK => {}
        r => panic!("ERC20 test transaction failed: {:?}", r),
    }

    // Print out the transaction's event log
    {
        use groundhog_erc20_if::*;
        use sdk::EmittedEvent;

        for (i, event) in res.events.iter().enumerate() {
            let event: EmittedEvent = event.into();
            match ERC20Event::try_from(event) {
                Ok(event) => eprintln!("\tevent {:?}: {}", i, event),
                Err(_) => eprintln!("\tevent {:?}: <non-ERC20 event>", i),
            }
        }
    }

    // Check that the event log matches the model
    {
        let m = model.lock().unwrap().clone().unwrap();
        m.check_events(&res.events);
    }
}
