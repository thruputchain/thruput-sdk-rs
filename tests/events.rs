use sdk::event::*;
use sdk::internal::decode_xdr;
use sdk::types::event::TxEmittedEvent;
use sdk::*;

macro_rules! single_event_test {
    {
        fn $test_name:ident() $body:tt
        fn event_check($event:ident) $body_event_check:tt
    } =>
    {
        #[test]
        fn $test_name() {
            let env = $crate::TestEnvironment::no_cost();
            let mut handler = contract_test_handler(env, || { $body });

            let res = handler.root_invoke();
            if res.output.status_code()
                != sdk::types::transaction::InvocationFrameStatusCode::Return
            {
                panic!("Invocation failed with code {}", res.output.status_code());
            }

            match handler.try_commit_transaction() {
                ExecResult::ExecResultOK => {}
                r => panic!("Transaction failed: {:?}", r),
            }

            let events = &res.events;

            assert_eq!(events.len(), 1);
            let event = &events[0];

            let event_check = |$event: &TxEmittedEvent| $body_event_check;
            event_check(event);
        }
    };
}

//-------------------------------------------------------------------------
// Log an event with the raw syscall
single_event_test! {
    fn test_event_no_topics() {
        sys::raw_LogEvent(None, None, None, b"test").unwrap();
    }

    fn event_check(event) {
        assert_eq!(event.num_topics, 0);
        assert_eq!(event.serialized_event.len(), 4);
    }
}

//-------------------------------------------------------------------------
// An event structure using owned data
#[derive(Debug, Default, PartialEq, WriteXDR, ReadXDR)]
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
    fn num_topics(&self) -> u32 {
        2
    }

    fn topic<'a>(&'a self, index: u32) -> Option<TopicSrc<'a>> {
        match index {
            0 => {
                // Here we need to hash something we don't have a reference to:
                let payer_payee =
                    (&self.payer, &self.payee).hash().into_inner();

                Some(TopicSrc::Owned(payer_payee))
            }
            1 => Some(TopicSrc::Owned(self.currency.hash().into_inner())),
            _ => None,
        }
    }
}

single_event_test! {
    fn test_owned_event() {
        let event = OwnedEvent::default();
        log_event(&event);
    }
    fn event_check(event) {
        assert_eq!(event.num_topics, 2);

        let payload = event_payload(event);
        let event: OwnedEvent = decode_xdr(payload).unwrap();
        assert_eq!(event, OwnedEvent::default());
    }
}

//-------------------------------------------------------------------------
// An event structure using pointers to minimize copying
#[allow(unused)]
#[derive(WriteXDR, ReadXDR)]
struct BorrowedEvent<'a> {
    payer: &'a ContractAddress,
    payee: &'a ContractAddress,
    currency: &'a [u8],

    amount: u128,
    memo: &'a [u8],
}

impl<'a> Event for BorrowedEvent<'a> {
    fn num_topics(&self) -> u32 {
        3
    }

    fn topic<'t>(&'t self, index: u32) -> Option<TopicSrc<'t>> {
        match index {
            0 => return Some(TopicSrc::Shared(self.payer)),
            1 => return Some(TopicSrc::Shared(self.payee)),
            2 => {
                return Some(TopicSrc::Owned(
                    self.currency.hash().into_inner(),
                ));
            }
            _ => return None,
        }
    }
}

single_event_test! {
    fn test_borrowed_event() {
        let payer = ContractAddress::default();
        let payee = ContractAddress::default();
        let currency = "USD".as_bytes();
        let amount = 33;
        let memo = "dinner".as_bytes();
        let event = BorrowedEvent { payer: &payer, payee: &payee, currency, amount, memo };

        log_event(&event);
    }
    fn event_check(event) {
        assert_eq!(event.num_topics, 3);

        let payload = event_payload(event);
        let event: BorrowedEvent = decode_xdr(payload).unwrap();
        assert_eq!(event.memo, "dinner".as_bytes());
    }
}

// utilities

fn event_payload(event: &TxEmittedEvent) -> &[u8] {
    let topics_len = 32 * event.num_topics as usize;

    &event.serialized_event[topics_len..]
}
