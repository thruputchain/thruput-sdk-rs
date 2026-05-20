mod common;
test_cfg!();

use sdk::call::RawCallError;
use sdk::event::*;
use sdk::exn::{Exn, HandlingError, LibraryExn};
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

#[derive(Event)]
struct LargePayloadEvent<'a> {
    data: &'a [u8],
}

#[derive(Debug, PartialEq, WriteXDR, ReadXDR)]
struct ManualTooManyTopicsEvent {
    value: u32,
}

impl Event for ManualTooManyTopicsEvent {
    type Payload<'a> = ManualTooManyTopicsEvent;

    fn num_topics(&self) -> u32 {
        4
    }

    fn topic<'a>(&'a self, index: u32) -> Option<TopicSrc<'a>> {
        match index {
            0 | 1 | 2 | 3 => Some(TopicSrc::Owned([index as u8; 32])),
            _ => None,
        }
    }

    fn decode_payload<'a>(
        bytes: &'a [u8],
    ) -> Result<ManualTooManyTopicsEvent, ReadError> {
        decode_xdr(bytes)
    }
}

fn assert_encoding_event_exception(result: Result<(), RawCallError>) {
    match result {
        Err(RawCallError::Exception(exn))
            if exn.selector() == LibraryExn::SELECTOR =>
        {
            match exn.try_as::<LibraryExn>() {
                Some(LibraryExn::Handling(HandlingError::EncodingEvent(
                    _,
                ))) => {}
                other => panic!(
                    "expected LibraryExn::Handling(EncodingEvent), got {:?}",
                    other
                ),
            }
        }
        other => panic!(
            "expected LibraryExn encoding exception for event handling, got {:?}",
            other
        ),
    }
}

#[test]
fn test_large_event_payload_within_runtime_limit_succeeds() {
    let env = TestEnvironment::no_cost();
    let payload = [0u8; 600];

    let result = env.transact(move |tx| {
        tx.call(&TEST_ADDR).run(|| {
            log_event(&LargePayloadEvent { data: &payload });
        })
    });

    result.assert_success();
    result.assert_event_count(1);
    result.assert_event::<LargePayloadEvent>(0, |e| {
        assert_eq!(e.payload().data, payload.as_slice());
    });
}

#[test]
fn test_manual_event_with_too_many_topics_is_rejected() {
    let env = TestEnvironment::no_cost();

    assert_encoding_event_exception(contract_exec(env, || {
        log_event(&ManualTooManyTopicsEvent { value: 7 });
    }));
}

#[test]
fn test_event_payload_exceeding_runtime_limit_is_rejected() {
    let env = TestEnvironment::no_cost();
    let payload = [0u8; EVENT_MAX_DATA as usize + 1];

    assert_encoding_event_exception(contract_exec(env, move || {
        log_event(&LargePayloadEvent { data: &payload });
    }));
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

    result.assert_event::<DerivedEvent>(0, |e| {
        assert_eq!(e.num_topics(), 3);
        assert_eq!(e.topic(0).unwrap(), DerivedEvent::SIGNATURE);
        assert_eq!(e.topic(1).unwrap(), &*ContractAddress::default());
        assert_eq!(e.topic(2).unwrap(), &*ContractAddress::default());
        assert_eq!(e.payload().amount, 42);
        assert_eq!(e.payload().memo, b"hello");
    });
}

#[test]
fn test_simple_event_payload_decoding() {
    let env = TestEnvironment::no_cost();

    let result = env.transact(|tx| {
        tx.call(&TEST_ADDR).run(|| {
            log_event(&SimpleEvent { value: 99 });
        })
    });

    result.assert_success();
    result.assert_event_count(1);

    result.assert_event::<SimpleEvent>(0, |e| {
        assert_eq!(e.topic(0).unwrap(), SimpleEvent::SIGNATURE);
        assert_eq!(e.payload().value, 99);
    });
}

//-------------------------------------------------------------------------
// Multi-lifetime event: both lifetimes unify to the decode buffer lifetime

//-------------------------------------------------------------------------
// Non-32-byte topic types (hashed via IntoTopic impls)

#[derive(Event)]
struct HashedTopicEvent<'a> {
    #[topic]
    category: &'a str,
    #[topic]
    amount: u128,
    memo: &'a [u8],
}

#[test]
fn test_hashed_topic_event() {
    let env = TestEnvironment::no_cost();

    let result = env.transact(|tx| {
        tx.call(&TEST_ADDR).run(|| {
            log_event(&HashedTopicEvent {
                category: "transfer",
                amount: 1000,
                memo: b"test",
            });
        })
    });

    result.assert_success();

    // Compute expected topic hashes outside the VM using sha2 directly.
    // hash_to_topic does: XDR-encode → SHA-256
    use sha2::{Digest, Sha256};
    let expected_category: [u8; 32] = {
        let xdr = sdk::internal::encode_xdr(&"transfer").unwrap();
        Sha256::digest(xdr.as_ref()).into()
    };
    let expected_amount: [u8; 32] = {
        let xdr = sdk::internal::encode_xdr(&1000u128).unwrap();
        Sha256::digest(xdr.as_ref()).into()
    };

    result.assert_event::<HashedTopicEvent>(0, |e| {
        assert_eq!(e.num_topics(), 3);
        assert_eq!(e.topic(0).unwrap(), HashedTopicEvent::SIGNATURE);
        assert_eq!(e.topic(1).unwrap(), expected_category.as_ref());
        assert_eq!(e.topic(2).unwrap(), expected_amount.as_ref());
        assert_eq!(e.payload().memo, b"test");
    });
}

//-------------------------------------------------------------------------
// Multi-lifetime event: both lifetimes unify to the decode buffer lifetime

#[derive(Event)]
struct MultiLifetimeEvent<'a, 'b> {
    #[topic]
    from: &'a ContractAddress,
    note_a: &'a [u8],
    note_b: &'b [u8],
    amount: u128,
}

#[test]
fn test_multi_lifetime_event() {
    let env = TestEnvironment::no_cost();

    let result = env.transact(|tx| {
        tx.call(&TEST_ADDR).run(|| {
            let addr = ContractAddress::default();
            log_event(&MultiLifetimeEvent {
                from: &addr,
                note_a: b"aaa",
                note_b: b"bbb",
                amount: 77,
            });
        })
    });

    result.assert_success();
    result.assert_event_count(1);

    result.assert_event::<MultiLifetimeEvent>(0, |e| {
        assert_eq!(e.num_topics(), 2); // signature + from
        assert_eq!(e.topic(0).unwrap(), MultiLifetimeEvent::SIGNATURE);
        assert_eq!(e.payload().note_a, b"aaa");
        assert_eq!(e.payload().note_b, b"bbb");
        assert_eq!(e.payload().amount, 77);
    });
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
