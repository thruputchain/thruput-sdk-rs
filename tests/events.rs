use native_exec::transaction_handler::TransactionHandler;
use sdk::event::*;
use sdk::*;
use std::rc::Rc;

// Log an event with the raw syscall
contract_test_env! {
    fn env_hook(env, handler) {
        grant_storage_tokens(env, handler);
    }

    fn test_event_no_topics() {
        println!("logging event");
        sys::raw_LogEvent(None, None, None, b"test").unwrap();
    }

    fn post_hook(handler) {
        let event_log = handler.get_event_logs().unwrap();
        let events = &event_log.events;

        assert_eq!(events.len(), 1);
    }
}


// An event structure using owned data
#[derive(Default, WriteXDR)]
struct OwnedEvent {
    // Things that become topics:
    payer: ContractAddress,
    payee: ContractAddress,
    currency: String,

    // Additional data:
    amount: u128,
    memo: String,
}

impl Event for OwnedEvent {
    #[cfg(not(feature = "topic_by_ref"))]
    // This interface allows us to construct arbitrary hashes
    fn topic(&self, index: u32) -> Option<Hash256> {
        let payer_payee = (&self.payer, &self.payee).hash();

        match index {
            0 => Some(payer_payee),
            1 => Some(self.currency.hash()),
            _ => None,
        }
    }

    #[cfg(feature = "topic_by_ref")]
    fn topic_by_ref(&self, index: u32) -> Option<&dyn Hash> {
        match index {
            0 => return Some(&self.payer),
            1 => return Some(&self.payee),
            2 => return Some(&self.currency),
            _ => return None,
        }
    }
}

contract_test_env! {
    fn env_hook(env, handler) {
        grant_storage_tokens(env, handler);
    }

    fn test_event_owned() {
        println!("logging event");

        let event = OwnedEvent::default();

        log_event(&event).unwrap();
    }

    fn post_hook(handler) {
        let event_log = handler.get_event_logs().unwrap();
        let events = &event_log.events;

        assert_eq!(events.len(), 1);
    }
}

// An event structure using pointers to minimize copying
#[allow(unused)]
#[derive(WriteXDR)]
struct BorrowedEvent<'a> {
    payer: &'a ContractAddress,
    payee: &'a ContractAddress,
    currency: &'a str,

    amount: u128,
    memo: &'a str,
}

impl<'a> Event for BorrowedEvent<'a> {
    #[cfg(not(feature = "topic_by_ref"))]
    fn topic(&self, index: u32) -> Option<Hash256> {
        let payer_payee = (self.payer, self.payee).hash();

        match index {
            0 => Some(payer_payee),
            1 => Some(self.currency.hash()),
            _ => None,
        }
    }

    #[cfg(feature = "topic_by_ref")]
    fn topic_by_ref(&self, index: u32) -> Option<&dyn Hash> {
        match index {
            0 => return Some(self.payer),
            1 => return Some(self.payee),
            2 => return Some(&self.currency),
            _ => return None,
        }
    }
}

contract_test_env! {
    fn env_hook(env, handler) {
        grant_storage_tokens(env, handler);
    }

    fn test_borrowed_event() {
        println!("logging event");

        let payer = ContractAddress::default();
        let payee = ContractAddress::default();
        let currency = "USD";
        let amount = 33;
        let memo = "dinner";
        let event = BorrowedEvent { payer: &payer, payee: &payee, currency, amount, memo };

        log_event(&event).unwrap();
    }

    fn post_hook(handler) {
        let event_log = handler.get_event_logs().unwrap();
        let events = &event_log.events;

        assert_eq!(events.len(), 1);
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
