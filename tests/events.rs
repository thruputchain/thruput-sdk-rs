use native_exec::transaction_handler::TransactionHandler;
use sdk::event::*;
use sdk::*;
use std::rc::Rc;

macro_rules! single_event_test {
    { fn $name:ident() $body:tt } =>
    {
        contract_test_env! {
            fn env_hook(env, handler) {
                grant_storage_tokens(env, handler);
            }

            fn $name() $body

            fn post_hook(handler) {
                let event_log = handler.get_event_logs().unwrap();
                let events = &event_log.events;

                assert_eq!(events.len(), 1);
            }
        }
    };
}

// Log an event with the raw syscall
single_event_test! {
    fn test_event_no_topics() {
        sys::raw_LogEvent(None, None, None, b"test").unwrap();
    }
}

//-------------------------------------------------------------------------
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
    // The `topic` method allows us to construct arbitrary hashes
    fn topic(&self, index: u32) -> Option<Hash256> {
        let payer_payee = (&self.payer, &self.payee).hash();

        match index {
            0 => Some(payer_payee),
            1 => Some(self.currency.hash()),
            _ => None,
        }
    }
}

single_event_test! {
    fn test_owned_event() {
        let event = OwnedEvent::default();
        log_event(&event).unwrap();
    }
}

//-------------------------------------------------------------------------
// An event structure using owned data
#[derive(Default, WriteXDR)]
struct OwnedEvent2 {
    // Things that become topics:
    payer: ContractAddress,
    payee: ContractAddress,
    currency: String,

    // Additional data:
    amount: u128,
    memo: String,
}

impl Event for OwnedEvent2 {
    // The `topic_by_ref` method may save some copying, in the case
    // that we can reference topic data in `self`:
    fn topic_by_ref(&self, index: u32) -> Option<&dyn Hash> {
        match index {
            0 => return Some(&self.payer),
            1 => return Some(&self.payee),
            2 => return Some(&self.currency),
            _ => return None,
        }
    }
}

single_event_test! {
    fn test_owned_event2() {
        let event = OwnedEvent2::default();
        log_event(&event).unwrap();
    }
}

//-------------------------------------------------------------------------
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
    // This method allows us to construct arbitrary hashes
    fn topic(&self, index: u32) -> Option<Hash256> {
        let payer_payee = (self.payer, self.payee).hash();

        match index {
            0 => Some(payer_payee),
            1 => Some(self.currency.hash()),
            _ => None,
        }
    }
}

single_event_test! {
    fn test_borrowed_event() {
        let payer = ContractAddress::default();
        let payee = ContractAddress::default();
        let currency = "USD";
        let amount = 33;
        let memo = "dinner";
        let event = BorrowedEvent { payer: &payer, payee: &payee, currency, amount, memo };

        log_event(&event).unwrap();
    }
}

//-------------------------------------------------------------------------
// An event structure using pointers to minimize copying
#[allow(unused)]
#[derive(WriteXDR)]
struct BorrowedEvent2<'a> {
    payer: &'a ContractAddress,
    payee: &'a ContractAddress,
    currency: &'a str,

    amount: u128,
    memo: &'a str,
}

impl<'a> Event for BorrowedEvent2<'a> {
    // This way may save copying
    fn topic_by_ref(&self, index: u32) -> Option<&dyn Hash> {
        match index {
            0 => return Some(self.payer),
            1 => return Some(self.payee),
            2 => return Some(&self.currency),
            _ => return None,
        }
    }
}

single_event_test! {
    fn test_borrowed_event2() {
        let payer = ContractAddress::default();
        let payee = ContractAddress::default();
        let currency = "USD";
        let amount = 33;
        let memo = "dinner";
        let event = BorrowedEvent { payer: &payer, payee: &payee, currency, amount, memo };

        log_event(&event).unwrap();
    }
}

// utilities

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
