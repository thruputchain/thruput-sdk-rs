use native_exec::transaction_handler::TransactionHandler;
use sdk::event::*;
use sdk::*;
use std::rc::Rc;

// An event structure using owned data
#[allow(unused)]
struct OwnedEvent {
    // Things that become topics:
    payer: ContractAddress,
    payee: ContractAddress,
    currency: String,

    data: EventData,
}

#[derive(Clone, WriteXDR)]
struct EventData {
    amount: u128,
    memo: String,
}

impl Event for OwnedEvent {
    type Data = EventData;

    fn topic(&self, index: u32) -> Option<Hash256> {
        let payer_payee = (&self.payer, &self.payee).hash();

        match index {
            0 => return Some(payer_payee),
            1 => return Some(self.currency.hash()),
            _ => return None,
        }
    }

    fn data(&self) -> Self::Data {
        self.data.clone()
    }
}

// An event structure using pointers to avoid excessive copying
#[allow(unused)]
struct BorrowedEvent<'a> {
    payer: &'a ContractAddress,
    payee: &'a ContractAddress,
    currency: &'a str,

    amount: u128,
    memo: &'a str,
}

impl<'a> Event for BorrowedEvent<'a> {
    type Data = (u128, &'a str);

    fn topic_by_ref(&self, index: u32) -> Option<&dyn Hash> {
        match index {
            0 => return Some(self.payer),
            1 => return Some(self.payee),
            2 => return Some(&self.currency),
            _ => return None,
        }
    }

    fn data(&self) -> Self::Data {
        (self.amount, &self.memo)
    }
}

contract_test_env! {
    fn env_hook(env, handler) {
        grant_storage_tokens(env, handler);
    }

    fn test_event() {
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
