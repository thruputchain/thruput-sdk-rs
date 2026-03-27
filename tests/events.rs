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
    type Payload<'a> = OwnedEvent;

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

    fn decode_payload<'a>(bytes: &'a [u8]) -> Result<OwnedEvent, ReadError> {
        decode_xdr(bytes)
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
    type Payload<'p> = BorrowedEvent<'p>;

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

    fn decode_payload<'p>(
        bytes: &'p [u8],
    ) -> Result<BorrowedEvent<'p>, ReadError> {
        decode_xdr(bytes)
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

//-------------------------------------------------------------------------
// An event using #[derive(Event)]

#[derive(Event)]
struct DerivedEvent<'a> {
    #[topic]
    payer: &'a ContractAddress,
    #[topic]
    payee: &'a ContractAddress,
    amount: u128,
    memo: &'a [u8],
}

single_event_test! {
    fn test_derived_event() {
        let payer = ContractAddress::default();
        let payee = ContractAddress::default();
        let event = DerivedEvent {
            payer: &payer,
            payee: &payee,
            amount: 42,
            memo: b"hello",
        };

        log_event(&event);
    }
    fn event_check(event) {
        // 3 topics: signature + payer + payee
        assert_eq!(event.num_topics, 3);

        // Topic 0 is the SIGNATURE
        let emitted = EmittedEvent::from(event);
        assert_eq!(emitted.topic(0).unwrap(), DerivedEvent::SIGNATURE);

        // Topics 1 and 2 are the payer/payee addresses
        assert_eq!(
            emitted.topic(1).unwrap(),
            &*ContractAddress::default()
        );
        assert_eq!(
            emitted.topic(2).unwrap(),
            &*ContractAddress::default()
        );

        // Payload is only amount + memo (not the topic fields)
        let payload = event_payload(event);

        // Decode just the amount and memo
        #[derive(Debug, PartialEq, ReadXDR)]
        struct Payload<'a> {
            amount: u128,
            memo: &'a [u8],
        }

        let decoded: Payload = decode_xdr(payload).unwrap();
        assert_eq!(decoded.amount, 42);
        assert_eq!(decoded.memo, b"hello");
    }
}

#[derive(Event)]
struct SimpleEvent {
    value: u128,
}

single_event_test! {
    fn test_derived_event_no_topics() {
        let event = SimpleEvent { value: 99 };
        log_event(&event);
    }
    fn event_check(event) {
        // 1 topic: just the signature
        assert_eq!(event.num_topics, 1);

        let emitted = EmittedEvent::from(event);
        assert_eq!(emitted.topic(0).unwrap(), SimpleEvent::SIGNATURE);

        let payload = event_payload(event);
        let decoded: u128 = decode_xdr(payload).unwrap();
        assert_eq!(decoded, 99);
    }
}

//-------------------------------------------------------------------------
// Field ordering: rearranging topic/payload fields should not change
// the serialized event, as long as topics stay in the same relative
// order and payload fields stay in the same relative order.

// Topics first, then payload
#[derive(Event)]
struct FieldOrderA<'a> {
    #[topic]
    from: &'a ContractAddress,
    #[topic]
    to: &'a ContractAddress,
    amount: u128,
    memo: &'a [u8],
}

// Interleaved: topic, payload, topic, payload
#[derive(Event)]
struct FieldOrderB<'a> {
    #[topic]
    from: &'a ContractAddress,
    amount: u128,
    #[topic]
    to: &'a ContractAddress,
    memo: &'a [u8],
}

// Payload first, then topics
#[derive(Event)]
struct FieldOrderC<'a> {
    amount: u128,
    memo: &'a [u8],
    #[topic]
    from: &'a ContractAddress,
    #[topic]
    to: &'a ContractAddress,
}

#[test]
fn test_field_order_does_not_affect_serialization() {
    let env = TestEnvironment::no_cost();
    let mut handler = contract_test_handler(env, || {
        let from = ContractAddress::default();
        let to = ContractAddress::default();

        log_event(&FieldOrderA {
            from: &from,
            to: &to,
            amount: 42,
            memo: b"hello",
        });
        log_event(&FieldOrderB {
            from: &from,
            amount: 42,
            to: &to,
            memo: b"hello",
        });
        log_event(&FieldOrderC {
            amount: 42,
            memo: b"hello",
            from: &from,
            to: &to,
        });
    });

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
    assert_eq!(events.len(), 3);

    // All three events should have the same number of topics
    assert_eq!(events[0].num_topics, events[1].num_topics);
    assert_eq!(events[0].num_topics, events[2].num_topics);

    // Topic 0 differs (it's the struct name hash), so compare
    // topics 1..N and payload — those must be identical.
    fn after_sig(e: &TxEmittedEvent) -> &[u8] {
        &e.serialized_event[32..]
    }

    assert_eq!(
        after_sig(&events[0]),
        after_sig(&events[1]),
        "interleaved field order changed serialization"
    );
    assert_eq!(
        after_sig(&events[0]),
        after_sig(&events[2]),
        "reversed field order changed serialization"
    );
}

//-------------------------------------------------------------------------
// Tests using the fluent MockTransactionHandler API

const TEST_ADDR: ContractAddress = ContractAddress::new([1u8; 32]);

#[test]
fn test_derived_event_fluent_api() {
    let env = TestEnvironment::no_cost();

    let result = env.transact(|tx| {
        tx.call(&TEST_ADDR).run(|| {
            let payer = ContractAddress::default();
            let payee = ContractAddress::default();
            log_event(&DerivedEvent {
                payer: &payer,
                payee: &payee,
                amount: 42,
                memo: b"hello",
            });
        })
    });

    result.assert_success();
    result.assert_event_count(1);

    // Check topics via raw event inspection
    result.assert_event(0, |e| {
        assert_eq!(e.num_topics, 3);

        let emitted = EmittedEvent::from(e);
        assert_eq!(emitted.topic(0).unwrap(), DerivedEvent::SIGNATURE);
        assert_eq!(emitted.topic(1).unwrap(), &*ContractAddress::default());
        assert_eq!(emitted.topic(2).unwrap(), &*ContractAddress::default());
    });

    // Check payload via borrowed decoding
    #[derive(Debug, PartialEq, ReadXDR)]
    struct Payload<'a> {
        amount: u128,
        memo: &'a [u8],
    }

    let payload = event_payload(&result.events()[0]);
    let decoded: Payload = decode_xdr(payload).unwrap();
    assert_eq!(decoded.amount, 42);
    assert_eq!(decoded.memo, b"hello");
}

#[test]
fn test_field_order_fluent_api() {
    let env = TestEnvironment::no_cost();

    let result = env.transact(|tx| {
        tx.call(&TEST_ADDR).run(|| {
            let from = ContractAddress::default();
            let to = ContractAddress::default();

            log_event(&FieldOrderA {
                from: &from,
                to: &to,
                amount: 42,
                memo: b"hello",
            });
            log_event(&FieldOrderB {
                from: &from,
                amount: 42,
                to: &to,
                memo: b"hello",
            });
            log_event(&FieldOrderC {
                amount: 42,
                memo: b"hello",
                from: &from,
                to: &to,
            });
        })
    });

    result.assert_success();
    result.assert_event_count(3);

    let events = result.events();

    // All three have the same number of topics
    assert_eq!(events[0].num_topics, events[1].num_topics);
    assert_eq!(events[0].num_topics, events[2].num_topics);

    // Topics 1..N and payload are identical (topic 0 differs by struct name)
    fn after_sig(e: &TxEmittedEvent) -> &[u8] {
        &e.serialized_event[32..]
    }

    assert_eq!(
        after_sig(&events[0]),
        after_sig(&events[1]),
        "interleaved field order changed serialization"
    );
    assert_eq!(
        after_sig(&events[0]),
        after_sig(&events[2]),
        "reversed field order changed serialization"
    );
}

// utilities

fn event_payload(event: &TxEmittedEvent) -> &[u8] {
    let topics_len = 32 * event.num_topics as usize;

    &event.serialized_event[topics_len..]
}
