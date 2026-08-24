use groundhog_native_exec::TryCommitResult;
use groundhog_native_exec::frame_result::FrameResult;
use groundhog_native_exec::transact_result::TransactResult;
use groundhog_xdr::exec_result::ExecResult;
use groundhog_xdr::transaction::InvocationFrameOutput;
use groundhog_xdr::types::ContractAddress;

use groundhog_event::{Event, TopicSrc};

/// The default contract address used by most tests.
const TEST_ADDR: ContractAddress = ContractAddress::new([1u8; 32]);

/// Wrapper for u32 in event tests. Declares 0 topics,
/// so topic filtering matches raw events with 0 topics.
#[derive(Debug, Default, PartialEq, xdrs::WriteXDR, xdrs::ReadXDR)]
struct TestValue(u32);

impl Event for TestValue {
    type Payload<'a> = TestValue;
    fn num_topics() -> u32 {
        0
    }
    fn signature() -> Option<&'static [u8; 32]> {
        None // no topics, so no discriminant
    }
    fn decode_payload(bytes: &[u8]) -> Result<TestValue, xdrs::ReadError> {
        xdrs::cxs::decode(bytes)
    }
}

const EVENT_A_SIG: [u8; 32] = [0xAA; 32];
const EVENT_B_SIG: [u8; 32] = [0xBB; 32];

/// Event type with topic 0 = EVENT_A_SIG
#[derive(Debug, Default, PartialEq, xdrs::WriteXDR, xdrs::ReadXDR)]
struct EventA(u32);

impl Event for EventA {
    type Payload<'a> = EventA;
    fn num_topics() -> u32 {
        1
    }
    fn topic(&self, index: u32) -> Option<TopicSrc<'_>> {
        match index {
            0 => Some(TopicSrc::Owned(EVENT_A_SIG)),
            _ => None,
        }
    }
    fn signature() -> Option<&'static [u8; 32]> {
        Some(&EVENT_A_SIG)
    }
    fn decode_payload(bytes: &[u8]) -> Result<EventA, xdrs::ReadError> {
        xdrs::cxs::decode(bytes)
    }
}

/// Different event type with topic 0 = EVENT_B_SIG but same payload shape
#[derive(Debug, Default, PartialEq, xdrs::WriteXDR, xdrs::ReadXDR)]
struct EventB(u32);

impl Event for EventB {
    type Payload<'a> = EventB;
    fn num_topics() -> u32 {
        1
    }
    fn topic(&self, index: u32) -> Option<TopicSrc<'_>> {
        match index {
            0 => Some(TopicSrc::Owned(EVENT_B_SIG)),
            _ => None,
        }
    }
    fn signature() -> Option<&'static [u8; 32]> {
        Some(&EVENT_B_SIG)
    }
    fn decode_payload(bytes: &[u8]) -> Result<EventB, xdrs::ReadError> {
        xdrs::cxs::decode(bytes)
    }
}

#[test]
fn test_frame_result_success() {
    let return_data = vec![1u8, 2, 3];
    let output = InvocationFrameOutput::Return {
        return_buffer: return_data.clone().try_into().unwrap(),
    };
    let result = FrameResult::new(output, None, Vec::new());

    assert!(result.is_success());
    assert!(!result.is_abort());
    assert!(!result.is_fault());
    result.assert_success();
    assert_eq!(result.return_buffer(), &[1, 2, 3]);
}

#[test]
fn test_frame_result_abort() {
    let abort_data = vec![4u8, 5, 6];
    let output = InvocationFrameOutput::Abort {
        abort_buffer: abort_data.clone().try_into().unwrap(),
    };
    let result = FrameResult::new(output, None, Vec::new());

    assert!(result.is_abort());
    assert!(!result.is_success());
    result.assert_abort();
    assert_eq!(result.abort_buffer(), &[4, 5, 6]);
}

#[test]
fn test_frame_result_fault() {
    let output = InvocationFrameOutput::Fault;
    let result = FrameResult::new(output, None, Vec::new());

    assert!(result.is_fault());
    result.assert_fault();
}

#[test]
#[should_panic(expected = "expected success")]
fn test_frame_result_assert_success_panics_on_fault() {
    let output = InvocationFrameOutput::Fault;
    let result = FrameResult::new(output, None, Vec::new());
    result.assert_success();
}

#[test]
#[should_panic(expected = "expected abort")]
fn test_frame_result_assert_abort_panics_on_success() {
    let return_data = vec![];
    let output = InvocationFrameOutput::Return {
        return_buffer: return_data.try_into().unwrap(),
    };
    let result = FrameResult::new(output, None, Vec::new());
    result.assert_abort();
}

#[test]
fn test_transact_result_success() {
    let return_data = vec![];
    let output = InvocationFrameOutput::Return {
        return_buffer: return_data.try_into().unwrap(),
    };
    let frame = FrameResult::new(output, None, Vec::new());
    let result = TransactResult::new(frame, ExecResult::ExecResultOK);
    result.assert_success();
}

#[test]
fn test_transact_result_abort() {
    let abort_data = vec![];
    let output = InvocationFrameOutput::Abort {
        abort_buffer: abort_data.try_into().unwrap(),
    };
    let frame = FrameResult::new(output, None, Vec::new());
    let result =
        TransactResult::new(frame, ExecResult::ExecResultTxRootFrameReverted);
    result.assert_abort();
}

#[test]
fn test_transact_result_commit_failed() {
    let return_data = vec![];
    let output = InvocationFrameOutput::Return {
        return_buffer: return_data.try_into().unwrap(),
    };
    let frame = FrameResult::new(output, None, Vec::new());
    let result =
        TransactResult::new(frame, ExecResult::ExecResultTxDeterministicError);
    result.assert_commit_failed();
}

#[test]
#[should_panic(expected = "expected success")]
fn test_transact_result_assert_success_panics_on_abort() {
    let abort_data = vec![];
    let output = InvocationFrameOutput::Abort {
        abort_buffer: abort_data.try_into().unwrap(),
    };
    let frame = FrameResult::new(output, None, Vec::new());
    let result =
        TransactResult::new(frame, ExecResult::ExecResultTxRootFrameReverted);
    result.assert_success();
}

use groundhog_native_exec::frame_builder::FrameAction;
use groundhog_native_exec::mock_transaction_handler::MockTransactionHandler;
use groundhog_native_exec::test_environment::TestEnvironment;

#[test]
fn test_mock_handler_commit() {
    let env = TestEnvironment::new(None);
    let mut handler = MockTransactionHandler::new(env);

    handler
        .open_root_frame(TEST_ADDR)
        .with_invoke_flags(0)
        .build()
        .execute(|_| FrameAction::Close)
        .result();

    let exec_result = handler.try_commit_transaction();
    assert_eq!(exec_result, TryCommitResult::Ok);
}

#[test]
fn test_mock_handler_abort() {
    let env = TestEnvironment::new(None);
    let mut handler = MockTransactionHandler::new(env);

    handler
        .open_root_frame(TEST_ADDR)
        .with_invoke_flags(0)
        .build()
        .execute(|_| FrameAction::Close)
        .result();

    handler.abort_transaction();
}

#[test]
fn test_mock_handler_event_logs_empty() {
    let env = TestEnvironment::new(None);
    let mut handler = MockTransactionHandler::new(env);

    let result = handler
        .open_root_frame(TEST_ADDR)
        .with_invoke_flags(0)
        .build()
        .execute(|_| FrameAction::Close)
        .result();

    // No events emitted, should be empty
    assert!(result.events().is_empty());

    handler.try_commit_transaction();
}

// ── Panic handling in low-level API ───────────────────────────────────

#[test]
fn test_mock_handler_panic_produces_fault() {
    let env = TestEnvironment::new(None);
    let mut handler = MockTransactionHandler::new(env);

    let result = handler
        .open_root_frame(TEST_ADDR)
        .with_invoke_flags(0)
        .build()
        .execute(|_| {
            panic!("simulated error");
        })
        .result();

    result.assert_fault();
}

#[test]
fn test_mock_handler_panic_then_commit() {
    let env = TestEnvironment::new(None);
    let mut handler = MockTransactionHandler::new(env);

    let result = handler
        .open_root_frame(TEST_ADDR)
        .with_invoke_flags(0)
        .build()
        .execute(|_| {
            panic!("simulated error");
        })
        .result();

    std::println!("result: {:?}", result);

    result.assert_fault();

    let exec_result = handler.try_commit_transaction();
    assert_eq!(exec_result, TryCommitResult::TxDeterministicError);
}

#[test]
fn test_mock_handler_panic_then_abort() {
    let env = TestEnvironment::new(None);
    let mut handler = MockTransactionHandler::new(env);

    handler
        .open_root_frame(TEST_ADDR)
        .with_invoke_flags(0)
        .build()
        .execute(|_| {
            panic!("simulated error");
        })
        .result();

    handler.abort_transaction();
}

#[test]
fn test_transact_success() {
    let env = TestEnvironment::new(None);

    let result = env.transact(|tx| {
        tx.call(&[1u8; 32].into()).run(|| {
            let res = groundhog_xdr::syscalls::GetCurrentAddress();
            assert_eq!(
                res.unwrap(),
                groundhog_xdr::types::ContractAddress::from([1; 32])
            );
        })
    });

    result.assert_success();
}

#[test]
fn test_transact_fault() {
    let env = TestEnvironment::new(None);

    let result = env.transact(|tx| {
        tx.call(&[1u8; 32].into()).run(|| -> () {
            panic!("intentional fault");
        })
    });

    result.assert_fault();
}

#[test]
fn test_transact_manual_commit() {
    let env = TestEnvironment::new(None);

    let exec_result = env.transact_manual(|tx| {
        let frame_result = tx.call(&[1u8; 32].into()).run(|| {});
        frame_result.assert_success();
        tx.try_commit_transaction()
    });
    assert_eq!(exec_result, TryCommitResult::Ok);
}

#[test]
fn test_transact_manual_abort() {
    let env = TestEnvironment::new(None);

    let _ = env.transact_manual(|tx| {
        let frame_result = tx.call(&[1u8; 32].into()).run(|| -> () {
            panic!("this should fault");
        });
        frame_result.assert_fault();
        tx.abort_transaction();
    });
}

#[test]
fn test_transact_manual_inspect_before_commit() {
    let env = TestEnvironment::new(None);

    let (frame_result, exec_result) = env.transact_manual(|tx| {
        let frame_result = tx.call(&[1u8; 32].into()).run(|| {
            // Run some contract code
            let addr = groundhog_xdr::syscalls::GetCurrentAddress().unwrap();
            assert_eq!(
                addr,
                groundhog_xdr::types::ContractAddress::from([1; 32])
            );
        });

        // Inspect the frame result before deciding what to do
        let is_ok = frame_result.is_success();
        let exec_result = if is_ok {
            tx.try_commit_transaction()
        } else {
            tx.abort_transaction();
            TryCommitResult::TxRootFrameReverted
        };
        (frame_result, exec_result)
    });

    frame_result.assert_success();
    assert_eq!(exec_result, TryCommitResult::Ok);
}

#[test]
fn test_transaction_call_run_success() {
    let env = TestEnvironment::new(None);
    let mut tx = MockTransactionHandler::new(env);

    let frame_result = tx.call(&[1u8; 32].into()).run(|| {
        let res = groundhog_xdr::syscalls::GetCurrentAddress();
        assert_eq!(
            res.unwrap(),
            groundhog_xdr::types::ContractAddress::from([1; 32])
        );
    });

    frame_result.assert_success();
}

#[test]
fn test_transaction_call_run_fault() {
    let env = TestEnvironment::new(None);
    let mut tx = MockTransactionHandler::new(env);

    let frame_result = tx.call(&[1u8; 32].into()).run(|| -> () {
        panic!("intentional test fault");
    });

    frame_result.assert_fault();
}

// ── End-to-end validation tests ────────────────────────────────────────────

#[test]
fn test_multi_transaction_scenario() {
    let env = TestEnvironment::new(None);

    // Transaction 1: succeeds
    let result = env.transact(|tx| {
        tx.call(&[1u8; 32].into()).run(|| {
            // Contract code that succeeds
        })
    });
    result.assert_success();

    // Advance block
    env.advance_block();

    // Transaction 2: succeeds
    let result = env.transact(|tx| {
        tx.call(&[1u8; 32].into()).run(|| {
            // More contract code
        })
    });
    result.assert_success();

    // Transaction 3: faults
    let result = env.transact(|tx| {
        tx.call(&[1u8; 32].into()).run(|| -> () {
            panic!("simulated failure");
        })
    });
    result.assert_fault();
}

#[test]
fn test_frame_result_raw_access() {
    let env = TestEnvironment::new(None);

    let result = env.transact(|tx| tx.call(&[1u8; 32].into()).run(|| {}));

    // Can access raw output
    let _output = result.frame().output();
    let _exec = result.exec_result();
}

#[test]
fn test_transact_events_empty() {
    let env = TestEnvironment::new(None);

    let result = env.transact(|tx| {
        tx.call(&[1u8; 32].into()).run(|| {
            // No events emitted
        })
    });

    result.assert_success();
    assert!(result.events().is_empty());
}

#[test]
fn test_transact_abort_buffer_access() {
    let env = TestEnvironment::new(None);

    let result = env.transact(|tx| {
        tx.call(&[1u8; 32].into()).run(|| -> () {
            // Trigger an abort via the Return syscall with revert_frame=true
            groundhog_xdr::syscalls::raw_Return(&[42u8, 43, 44], true, &[]);
        })
    });

    result.assert_abort();
    // The abort buffer should contain the data we passed
    let buf = result.frame().abort_buffer();
    assert_eq!(buf, &[42, 43, 44]);
}

// ── Continuable frame tests ──────────────────────────────────────────────

// ── Per-frame event tests ─────────────────────────────────────────────

#[test]
fn test_frame_events_single() {
    let env = TestEnvironment::new(None);

    let result = env.transact(|tx| {
        tx.call(&[1u8; 32].into()).run(|| {
            groundhog_xdr::syscalls::raw_LogEvent(None, None, None, b"hello")
                .unwrap();
        })
    });

    result.assert_success();
    assert_eq!(result.events().len(), 1);
    result.assert_raw_event(0, |e| {
        assert_eq!(e.num_topics(), 0);
        assert_eq!(e.payload().unwrap(), b"hello");
    });
}

#[test]
fn test_frame_events_multiple() {
    let env = TestEnvironment::new(None);

    let result = env.transact(|tx| {
        tx.call(&[1u8; 32].into()).run(|| {
            groundhog_xdr::syscalls::raw_LogEvent(None, None, None, b"first")
                .unwrap();
            groundhog_xdr::syscalls::raw_LogEvent(None, None, None, b"second")
                .unwrap();
        })
    });

    result.assert_success();
    assert_eq!(result.events().len(), 2);
    result.assert_raw_event(0, |e| {
        assert_eq!(e.payload().unwrap(), b"first");
    });
    result.assert_raw_event(1, |e| {
        assert_eq!(e.payload().unwrap(), b"second");
    });
}

#[test]
fn test_frame_events_with_topics() {
    let env = TestEnvironment::new(None);

    let topic = groundhog_xdr::event::Topic::new([0xAA; 32]);

    let result = env.transact(|tx| {
        tx.call(&[1u8; 32].into()).run(|| {
            groundhog_xdr::syscalls::raw_LogEvent(
                Some(&groundhog_xdr::event::Topic::new([0xAA; 32])),
                None,
                None,
                b"data",
            )
            .unwrap();
        })
    });

    result.assert_success();
    assert_eq!(result.events().len(), 1);
    result.assert_raw_event(0, |e| {
        assert_eq!(e.num_topics(), 1);
        assert_eq!(e.topic(0).unwrap().as_ref(), topic.as_ref());
        assert_eq!(e.payload().unwrap(), b"data");
    });
}

#[test]
fn test_frame_events_contract_address() {
    let env = TestEnvironment::new(None);
    let addr = groundhog_xdr::types::ContractAddress::from([0x42; 32]);

    let result = env.transact(|tx| {
        tx.call(&addr).run(|| {
            groundhog_xdr::syscalls::raw_LogEvent(None, None, None, b"evt")
                .unwrap();
        })
    });

    result.assert_success();
    result.assert_raw_event(0, |e| {
        assert_eq!(e.raw().contract_address, addr);
    });
}

#[test]
fn test_frame_events_empty_on_fault() {
    let env = TestEnvironment::new(None);

    let result = env.transact(|tx| {
        tx.call(&[1u8; 32].into()).run(|| -> () {
            groundhog_xdr::syscalls::raw_LogEvent(
                None,
                None,
                None,
                b"before_fault",
            )
            .unwrap();
            panic!("fault after emitting event");
        })
    });

    result.assert_fault();
    // Events from a faulted frame are not returned
    assert!(result.events().is_empty());
}

#[test]
fn test_frame_events_empty_on_revert() {
    let env = TestEnvironment::new(None);

    let result = env.transact(|tx| {
        tx.call(&[1u8; 32].into()).run(|| -> () {
            groundhog_xdr::syscalls::raw_LogEvent(
                None,
                None,
                None,
                b"before_revert",
            )
            .unwrap();
            groundhog_xdr::syscalls::raw_Return(&[1], true, &[]);
        })
    });

    result.assert_abort();
    // Events from a reverted frame are not returned
    assert!(result.events().is_empty());
}

#[test]
fn test_frame_events_via_transact_manual() {
    let env = TestEnvironment::new(None);

    let frame_result = env.transact_manual(|tx| {
        let frame = tx.call(&[1u8; 32].into()).run(|| {
            groundhog_xdr::syscalls::raw_LogEvent(None, None, None, b"manual")
                .unwrap();
        });
        tx.try_commit_transaction();
        frame
    });

    frame_result.assert_success();
    assert_eq!(frame_result.events().len(), 1);
    frame_result.assert_raw_event(0, |e| {
        assert_eq!(e.payload().unwrap(), b"manual");
    });
}

// ── Typed event assertion tests ───────────────────────────────────────

#[test]
fn test_assert_event_data_u32() {
    let env = TestEnvironment::new(None);

    let result = env.transact(|tx| {
        tx.call(&[1u8; 32].into()).run(|| {
            let v: u32 = 12345;
            let data = xdrs::cxs::encode(&v).unwrap();
            groundhog_xdr::syscalls::raw_LogEvent(None, None, None, &data)
                .unwrap();
        })
    });

    result.assert_success();
    assert_eq!(result.events().len(), 1);

    // Decode the event payload as TestValue and check
    result.assert_event::<TestValue>(0, |e| {
        assert_eq!(e.payload().0, 12345);
    });

    // Also works on the frame directly
    result.frame().assert_event::<TestValue>(0, |e| {
        assert_eq!(e.payload().0, 12345);
    });
}

#[test]
fn test_assert_event_data_with_topic() {
    let env = TestEnvironment::new(None);

    let result = env.transact(|tx| {
        tx.call(&[1u8; 32].into()).run(|| {
            let v: u32 = 99;
            let data = xdrs::cxs::encode(&v).unwrap();
            groundhog_xdr::syscalls::raw_LogEvent(
                Some(&groundhog_xdr::event::Topic::new([0xBB; 32])),
                None,
                None,
                &data,
            )
            .unwrap();
        })
    });

    result.assert_success();
    // assert_event strips topics and decodes payload
    result.assert_event::<EventB>(0, |e| {
        assert_eq!(e.payload().0, 99);
    });
}

#[test]
fn test_assert_emitted_finds_matching_event() {
    let env = TestEnvironment::new(None);

    let result = env.transact(|tx| {
        tx.call(&[1u8; 32].into()).run(|| {
            for v in [10u32, 20, 30] {
                let data = xdrs::cxs::encode(&v).unwrap();
                groundhog_xdr::syscalls::raw_LogEvent(None, None, None, &data)
                    .unwrap();
            }
        })
    });

    result.assert_success();
    assert_eq!(result.events().len(), 3);

    // Find the event with value 20
    result.assert_emitted::<TestValue>(&TEST_ADDR, |t| t.payload().0 == 20);

    // Also works on the frame directly
    result
        .frame()
        .assert_emitted::<TestValue>(&TEST_ADDR, |t| t.payload().0 == 30);
}

#[test]
#[should_panic(expected = "expected exactly 1 matching event, got 0")]
fn test_assert_emitted_panics_when_no_match() {
    let env = TestEnvironment::new(None);

    let result = env.transact(|tx| {
        tx.call(&[1u8; 32].into()).run(|| {
            let data = xdrs::cxs::encode(&42u32).unwrap();
            groundhog_xdr::syscalls::raw_LogEvent(None, None, None, &data)
                .unwrap();
        })
    });

    result.assert_success();
    result.assert_emitted::<TestValue>(&TEST_ADDR, |t| t.payload().0 == 999);
}

#[test]
#[should_panic(expected = "expected exactly 1 matching event, got 0")]
fn test_assert_emitted_panics_on_empty_events() {
    let env = TestEnvironment::new(None);

    let result = env.transact(|tx| tx.call(&[1u8; 32].into()).run(|| {}));

    result.assert_success();
    result.assert_emitted::<TestValue>(&TEST_ADDR, |_| true);
}

#[test]
#[should_panic(expected = "event index 5 out of range")]
fn test_assert_event_out_of_range() {
    let env = TestEnvironment::new(None);

    let result = env.transact(|tx| {
        tx.call(&[1u8; 32].into()).run(|| {
            let data = xdrs::cxs::encode(&1u32).unwrap();
            groundhog_xdr::syscalls::raw_LogEvent(None, None, None, &data)
                .unwrap();
        })
    });

    result.assert_success();
    result.assert_event::<TestValue>(5, |_| {});
}

// ── Named addresses ──────────────────────────────────────────────────────

use groundhog_native_exec::test_helpers::Named;

#[test]
fn test_named_contract_address() {
    let alice = groundhog_xdr::types::ContractAddress::named("alice");
    let bob = groundhog_xdr::types::ContractAddress::named("bob");

    assert_eq!(alice, groundhog_xdr::types::ContractAddress::named("alice"));
    assert_ne!(alice, bob);
}

#[test]
fn test_named_address_in_transact() {
    let env = TestEnvironment::new(None);
    let alice = groundhog_xdr::types::ContractAddress::named("alice");

    let result = env.transact(|tx| {
        tx.call(&alice).run(|| {
            let addr = groundhog_xdr::syscalls::GetCurrentAddress().unwrap();
            assert_eq!(
                addr,
                groundhog_xdr::types::ContractAddress::named("alice")
            );
        })
    });
    result.assert_success();
}

// ── Return value assertions ──────────────────────────────────────────────

#[test]
fn test_assert_return_with() {
    let env = TestEnvironment::new(None);

    let result = env.transact(|tx| {
        tx.call(&[1u8; 32].into()).run(|| -> () {
            let v: u32 = 42;
            let data = xdrs::cxs::encode(&v).unwrap();
            groundhog_xdr::syscalls::raw_Return(&data, false, &[]);
        })
    });

    result.assert_success();
    result.assert_return_with::<u32>(|v| {
        assert_eq!(*v, 42);
    });
    result.frame().assert_return_with::<u32>(|v| {
        assert_eq!(*v, 42);
    });
}

// ── Convenience event assertions ─────────────────────────────────────────

#[test]
fn test_assert_event_count() {
    let env = TestEnvironment::new(None);

    let result = env.transact(|tx| {
        tx.call(&[1u8; 32].into()).run(|| {
            groundhog_xdr::syscalls::raw_LogEvent(None, None, None, b"a")
                .unwrap();
            groundhog_xdr::syscalls::raw_LogEvent(None, None, None, b"b")
                .unwrap();
        })
    });

    result.assert_success();
    result.assert_event_count(2);
    result.frame().assert_event_count(2);
}

#[test]
fn test_assert_no_events() {
    let env = TestEnvironment::new(None);

    let result = env.transact(|tx| tx.call(&[1u8; 32].into()).run(|| {}));

    result.assert_success();
    result.assert_no_events();
    result.frame().assert_no_events();
}

#[test]
fn test_filter_events() {
    let env = TestEnvironment::new(None);

    let result = env.transact(|tx| {
        tx.call(&[1u8; 32].into()).run(|| {
            for v in [10u32, 20, 30] {
                let data = xdrs::cxs::encode(&v).unwrap();
                groundhog_xdr::syscalls::raw_LogEvent(None, None, None, &data)
                    .unwrap();
            }
        })
    });

    result.assert_success();
    let values = result.filter_events::<TestValue>(&TEST_ADDR);
    let payloads: Vec<&TestValue> =
        values.iter().map(|t| t.payload()).collect();
    assert_eq!(
        payloads,
        vec![&TestValue(10), &TestValue(20), &TestValue(30)]
    );
}

// ── Continuable frame tests ──────────────────────────────────────────────

#[test]
fn test_run_continuable_basic() {
    let env = TestEnvironment::new(None);
    let mut tx = MockTransactionHandler::new(env);

    let result = tx
        .call(&[1u8; 32].into())
        .run_continuable(|_frame| {
            // First execution step
            FrameAction::Continue
        })
        .run_continuable(|_frame| {
            // Second execution step — close frame
            FrameAction::Close
        })
        .finish();

    result.assert_success();
}

#[test]
fn test_run_continuable_with_child_frame() {
    let env = TestEnvironment::new(None);
    let mut tx = MockTransactionHandler::new(env);

    let result = tx
        .call(&[1u8; 32].into())
        .run_continuable(|frame| {
            // Open a child frame from within the continuable frame
            let child_result = frame.child_call(&[2u8; 32].into()).run(|| {
                let addr =
                    groundhog_xdr::syscalls::GetCurrentAddress().unwrap();
                assert_eq!(
                    addr,
                    groundhog_xdr::types::ContractAddress::from([2; 32])
                );
            });
            child_result.assert_success();
            FrameAction::Close
        })
        .finish();

    result.assert_success();
}

#[test]
fn test_child_frame_return_value_decodes() {
    let env = TestEnvironment::new(None);
    let mut tx = MockTransactionHandler::new(env);

    let result = tx
        .call(&[1u8; 32].into())
        .run_continuable(|frame| {
            // Child frame returns a value; decode it from the child result.
            let child_result =
                frame.child_call(&[2u8; 32].into()).run(|| 7u32);
            assert_eq!(child_result.assert_return_as::<u32>(), 7);
            FrameAction::Close
        })
        .finish();

    result.assert_success();
}

// ── Gas limit regression test ─────────────────────────────────────────

#[test]
fn test_mock_handler_syscall_sha256() {
    let env = TestEnvironment::new(None);
    let mut handler = MockTransactionHandler::new(env);

    let result = handler
        .open_root_frame(TEST_ADDR)
        .with_invoke_flags(0)
        .build()
        .execute(|_| {
            // SHA256 consumes gas — this would fail with gas_limit: 0
            let input = b"hello world";
            let mut hash = groundhog_xdr::types::SHA256Hash::default();
            groundhog_xdr::syscalls::raw_SHA256(input.as_slice(), &mut hash)
                .unwrap();
            FrameAction::Close
        })
        .result();

    result.assert_success();
}

// ── Topic-aware filtering tests ───────────────────────────────────────

#[test]
fn test_filter_events_by_topic() {
    let env = TestEnvironment::new(None);

    let result = env.transact(|tx| {
        tx.call(&[1u8; 32].into()).run(|| {
            // Emit EventA(10) with topic 0 = EVENT_A_SIG
            let data = xdrs::cxs::encode(&10u32).unwrap();
            groundhog_xdr::syscalls::raw_LogEvent(
                Some(&groundhog_xdr::event::Topic::new(EVENT_A_SIG)),
                None,
                None,
                &data,
            )
            .unwrap();

            // Emit EventB(20) with topic 0 = EVENT_B_SIG
            let data = xdrs::cxs::encode(&20u32).unwrap();
            groundhog_xdr::syscalls::raw_LogEvent(
                Some(&groundhog_xdr::event::Topic::new(EVENT_B_SIG)),
                None,
                None,
                &data,
            )
            .unwrap();

            // Emit EventA(30) with topic 0 = EVENT_A_SIG
            let data = xdrs::cxs::encode(&30u32).unwrap();
            groundhog_xdr::syscalls::raw_LogEvent(
                Some(&groundhog_xdr::event::Topic::new(EVENT_A_SIG)),
                None,
                None,
                &data,
            )
            .unwrap();
        })
    });

    result.assert_success();
    assert_eq!(result.events().len(), 3);

    // filter_events<EventA> should only return the two EventA instances
    let a_events = result.filter_events::<EventA>(&TEST_ADDR);
    let a_payloads: Vec<&EventA> =
        a_events.iter().map(|t| t.payload()).collect();
    assert_eq!(a_payloads, vec![&EventA(10), &EventA(30)]);

    // filter_events<EventB> should only return the one EventB instance
    let b_events = result.filter_events::<EventB>(&TEST_ADDR);
    let b_payloads: Vec<&EventB> =
        b_events.iter().map(|t| t.payload()).collect();
    assert_eq!(b_payloads, vec![&EventB(20)]);

    // TestValue declares no signature but does declare 0 topics, so the
    // topic-count check rejects all three single-topic events even though
    // their payloads decode as u32. Pins the count check for
    // signature-less types.
    let untyped = result.filter_events::<TestValue>(&TEST_ADDR);
    assert_eq!(untyped.len(), 0);
}

#[test]
fn test_filter_events_topic_mismatch_skips() {
    let env = TestEnvironment::new(None);

    let result = env.transact(|tx| {
        tx.call(&[1u8; 32].into()).run(|| {
            // Emit an event with no topics
            let data = xdrs::cxs::encode(&42u32).unwrap();
            groundhog_xdr::syscalls::raw_LogEvent(None, None, None, &data)
                .unwrap();
        })
    });

    result.assert_success();

    // EventA declares a signature, but the raw event has no topics → no match
    let events = result.filter_events::<EventA>(&TEST_ADDR);
    assert!(events.is_empty());

    // TestValue declares 0 topics and the raw event has 0 → count matches,
    // and the payload decodes → match
    let events = result.filter_events::<TestValue>(&TEST_ADDR);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].payload(), &TestValue(42));
}

#[test]
fn test_assert_emitted_distinguishes_by_topic() {
    let env = TestEnvironment::new(None);

    let result = env.transact(|tx| {
        tx.call(&[1u8; 32].into()).run(|| {
            // Emit EventA(10)
            let data = xdrs::cxs::encode(&10u32).unwrap();
            groundhog_xdr::syscalls::raw_LogEvent(
                Some(&groundhog_xdr::event::Topic::new(EVENT_A_SIG)),
                None,
                None,
                &data,
            )
            .unwrap();

            // Emit EventB(20) — same payload shape, different topic
            let data = xdrs::cxs::encode(&20u32).unwrap();
            groundhog_xdr::syscalls::raw_LogEvent(
                Some(&groundhog_xdr::event::Topic::new(EVENT_B_SIG)),
                None,
                None,
                &data,
            )
            .unwrap();
        })
    });

    result.assert_success();

    // assert_emitted for EventA should find EventA(10) but not EventB(20)
    result.assert_emitted::<EventA>(&TEST_ADDR, |t| t.payload().0 == 10);

    // assert_emitted for EventB should find EventB(20) but not EventA(10)
    result.assert_emitted::<EventB>(&TEST_ADDR, |t| t.payload().0 == 20);
}

#[test]
#[should_panic(expected = "expected exactly 1 matching event, got 0")]
fn test_assert_emitted_rejects_wrong_topic() {
    let env = TestEnvironment::new(None);

    let result = env.transact(|tx| {
        tx.call(&[1u8; 32].into()).run(|| {
            // Only emit EventA
            let data = xdrs::cxs::encode(&10u32).unwrap();
            groundhog_xdr::syscalls::raw_LogEvent(
                Some(&groundhog_xdr::event::Topic::new(EVENT_A_SIG)),
                None,
                None,
                &data,
            )
            .unwrap();
        })
    });

    result.assert_success();

    // EventB should NOT match — topic 0 differs despite compatible payload
    result.assert_emitted::<EventB>(&TEST_ADDR, |_| true);
}

#[test]
fn test_assert_emitted_skips_unrelated_malformed_event() {
    use groundhog_xdr::event::TxEmittedEvent;

    // A valid TestValue(20) event sits alongside an unrelated, internally
    // malformed event from the same address: the malformed one claims 2
    // topics (64 bytes) but its buffer holds only ~4. assert_emitted must
    // skip the malformed event — exactly as filter_events does — and still
    // find the one genuine match, not abort on the corrupt sibling.
    let valid = TxEmittedEvent {
        contract_address: TEST_ADDR,
        num_topics: 0,
        serialized_event: xdrs::cxs::encode(&20u32)
            .unwrap()
            .try_into()
            .unwrap(),
    };
    let malformed = TxEmittedEvent {
        contract_address: TEST_ADDR,
        num_topics: 2,
        serialized_event: xdrs::cxs::encode(&99u32)
            .unwrap()
            .try_into()
            .unwrap(),
    };
    let output = InvocationFrameOutput::Return {
        return_buffer: Vec::new().try_into().unwrap(),
    };
    let result = FrameResult::new(output, None, vec![valid, malformed]);

    result.assert_emitted::<TestValue>(&TEST_ADDR, |t| t.payload().0 == 20);
}

//-------------------------------------------------------------------------
// Generic run(): closure return values travel through the return buffer.

#[test]
fn test_run_encodes_closure_return_value() {
    let env = TestEnvironment::new(None);
    let result = env.transact(|tx| tx.call(&TEST_ADDR).run(|| 42u32));
    result.assert_success();
    let decoded: u32 =
        xdrs::cxs::decode(result.frame().return_buffer()).unwrap();
    assert_eq!(decoded, 42);
}

#[test]
fn test_run_unit_closure_leaves_buffer_empty() {
    let env = TestEnvironment::new(None);
    let result = env.transact(|tx| tx.call(&TEST_ADDR).run(|| ()));
    result.assert_success();
    assert!(result.frame().return_buffer().is_empty());
}

#[test]
fn test_run_unit_closure_with_explicit_raw_return_is_preserved() {
    // Legacy style: unit closure sets the buffer itself. The wrapper must
    // not emit a second return (unit encodes to zero bytes -> skipped).
    let env = TestEnvironment::new(None);
    let result = env.transact(|tx| {
        tx.call(&TEST_ADDR).run(|| -> () {
            groundhog_xdr::syscalls::raw_Return(&[7u8, 8, 9], false, &[]);
        })
    });
    result.assert_success();
    assert_eq!(result.frame().return_buffer(), &[7, 8, 9]);
}

#[test]
fn test_run_tuple_return_roundtrip() {
    let env = TestEnvironment::new(None);
    let result = env
        .transact(|tx| tx.call(&TEST_ADDR).run(|| (1u64, String::from("hi"))));
    result.assert_success();
    let decoded: (u64, String) =
        xdrs::cxs::decode(result.frame().return_buffer()).unwrap();
    assert_eq!(decoded, (1, String::from("hi")));
}

#[test]
fn test_run_panic_produces_fault_not_value() {
    let env = TestEnvironment::new(None);
    let result = env
        .transact(|tx| tx.call(&TEST_ADDR).run(|| -> u32 { panic!("boom") }));
    result.assert_fault();
}

#[test]
fn test_run_nonunit_return_after_explicit_raw_return_pins_behavior() {
    // Unsupported mix: closure sets the buffer explicitly AND returns a
    // non-unit value. This test pins whatever the syscall layer does
    // (overwrite / fault) so a behavior change is caught.
    //
    // Observed: raw_Return never returns (it immediately terminates the
    // invocation), so the closure never produces the 42 and the wrapper
    // never encodes it. The explicit buffer wins.
    let env = TestEnvironment::new(None);
    let result = env.transact(|tx| {
        tx.call(&TEST_ADDR).run(|| {
            groundhog_xdr::syscalls::raw_Return(&[1u8], false, &[]);
            #[allow(unreachable_code)]
            42u32
        })
    });
    result.assert_success();
    assert_eq!(result.frame().return_buffer(), &[1]);
}

//-------------------------------------------------------------------------
// Typed buffer extraction: assert_return_as / assert_abort_as.

#[test]
fn test_assert_return_as_decodes_value() {
    let env = TestEnvironment::new(None);
    let result = env.transact(|tx| tx.call(&TEST_ADDR).run(|| 1234u64));
    result.assert_success();
    let v: u64 = result.assert_return_as();
    assert_eq!(v, 1234);
}

#[test]
#[should_panic(expected = "failed to decode return buffer")]
fn test_assert_return_as_panics_on_garbage() {
    let env = TestEnvironment::new(None);
    let result = env.transact(|tx| {
        tx.call(&TEST_ADDR).run(|| -> () {
            groundhog_xdr::syscalls::raw_Return(&[0xFF], false, &[]);
        })
    });
    let _: u64 = result.assert_return_as();
}

#[test]
fn test_assert_abort_as_decodes_error() {
    let env = TestEnvironment::new(None);
    let result = env.transact(|tx| {
        tx.call(&TEST_ADDR).run(|| -> () {
            let encoded = xdrs::cxs::encode(&77u32).unwrap();
            groundhog_xdr::syscalls::raw_Return(&encoded, true, &[]);
        })
    });
    result.assert_abort();
    let e: u32 = result.assert_abort_as();
    assert_eq!(e, 77);
}

/// Non-unit type whose XDR encoding is zero bytes.
#[derive(Debug, PartialEq, xdrs::WriteXDR, xdrs::ReadXDR)]
struct Empty;

#[test]
fn test_run_zero_byte_encoding_roundtrips() {
    let env = TestEnvironment::new(None);
    let result = env.transact(|tx| tx.call(&TEST_ADDR).run(|| Empty));
    result.assert_success();
    // Zero-byte encodings skip the explicit return, leaving the buffer
    // empty — which is itself the valid encoding, so decoding works.
    assert!(result.frame().return_buffer().is_empty());
    let v: Empty = result.assert_return_as();
    assert_eq!(v, Empty);
}

// ── Interleaving suspended subframes across transactions ──────────────

/// A subframe can be suspended (`FrameAction::Continue`) while another
/// transaction runs to completion, then resumed and closed — as long as it
/// is opened from the suspended parent's handle at top level
/// (`ContinuableFrame::child_call`), not from inside an `execute` closure.
/// The borrow checker enforces the frame-nesting discipline: while the
/// child's `ContinuableFrame` is alive, the parent cannot be executed,
/// closed, or given another child.
#[test]
fn test_subframe_suspends_across_interleaved_transactions() {
    let env = TestEnvironment::new(None);
    let mut tx1 = MockTransactionHandler::new(env.clone());
    let mut tx2 = MockTransactionHandler::new(env);

    let addr1 = ContractAddress::new([1u8; 32]);
    let addr2 = ContractAddress::new([2u8; 32]);
    let addr3 = ContractAddress::new([3u8; 32]);

    // tx1: root frame, suspended.
    let mut root = tx1.call(&addr1).run_continuable(|_| FrameAction::Continue);

    // tx1: subframe opened from the suspended parent, then suspended itself.
    let mut child = root
        .child_call(&addr2)
        .run_continuable(|_| FrameAction::Continue);

    // tx2 runs to completion while tx1's subframe is suspended.
    let r2 = tx2.call(&addr3).run(|| ());
    r2.assert_success();

    // Resume and close tx1 inner-first: the subframe, then the root.
    child.run_continuable(|_| FrameAction::Close);
    child.finish().assert_success();

    root.run_continuable(|_| FrameAction::Close);
    root.finish().assert_success();

    assert_eq!(tx1.try_commit_transaction(), TryCommitResult::Ok);
    assert_eq!(tx2.try_commit_transaction(), TryCommitResult::Ok);
}

#[test]
fn transact_manual_without_commit_panics() {
    // A closure that runs a frame but forgets try_commit_transaction /
    // abort_transaction is a test bug: the dropped handler would abort
    // silently and the test would pass with nothing committed. The harness
    // panics instead.
    let env = TestEnvironment::new(None);

    let caught =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            env.transact_manual(|tx| {
                tx.call(&[9u8; 32].into()).run(|| {}).assert_success();
                // ... and forgets to commit or abort.
            });
        }));

    let payload = caught.expect_err("forgetting to commit must panic");
    let msg = payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
        .expect("panic payload must be a message");
    assert!(
        msg.contains("transact_manual closure returned without finishing"),
        "unexpected message: {msg}"
    );
}
