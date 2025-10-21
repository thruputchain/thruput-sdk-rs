use native_exec::transaction_handler::TransactionHandler;
use sdk::event::*;
use sdk::*;
use std::rc::Rc;

#[allow(dead_code)]
#[derive(Default)]
struct MyEvent {
    payer: ContractAddress,
    payee: ContractAddress,
    amount: u128,
}

impl Event for MyEvent {
    fn topics(&self) -> [Option<&SHA256Hash>; EVENT_MAX_TOPICS] {
        [None; EVENT_MAX_TOPICS]
    }

    fn data(&self) -> impl WriteXDR {
        self.amount
    }
}

contract_test_env! {
    fn env_hook(env, handler) {
        grant_storage_tokens(env, handler);
    }

    fn test_event() {
        println!("logging event");

        let event = MyEvent::default();
        log_event(&event).unwrap();
    }
}

// Simple test to demonstrate reading events that were emitted by a contract.
contract_test_env! {
    fn env_hook(env, handler) {
        grant_storage_tokens(env, handler);
    }

    fn test_event_no_topics() {
        println!("logging event");
        sys::raw_LogEvent(None, None, None, b"test").unwrap();
    }
}

fn grant_storage_tokens(
    env: &Rc<TestEnvironment>,
    handler: &TransactionHandler,
) {
    env.setup_grant_storage_tokens(
        &handler
            .get_signed_transaction()
            .tx
            .invocation
            .invoked_address,
        1_000_000_000,
    );
}
