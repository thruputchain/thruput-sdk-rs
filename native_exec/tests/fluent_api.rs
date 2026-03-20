use groundhog_native_exec::frame_result::FrameResult;
use groundhog_native_exec::transact_result::TransactResult;
use groundhog_xdr::exec_result::ExecResult;
use groundhog_xdr::transaction::InvocationFrameOutput;

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
        .open_root_frame()
        .with_invoke_flags(0)
        .build()
        .execute(|_| FrameAction::Close)
        .result();

    let exec_result = handler.try_commit_transaction();
    assert_eq!(exec_result, ExecResult::ExecResultOK);
}

#[test]
fn test_mock_handler_abort() {
    let env = TestEnvironment::new(None);
    let mut handler = MockTransactionHandler::new(env);

    handler
        .open_root_frame()
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
        .open_root_frame()
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
        .open_root_frame()
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
        .open_root_frame()
        .with_invoke_flags(0)
        .build()
        .execute(|_| {
            panic!("simulated error");
        })
        .result();

    result.assert_fault();

    // A bare panic (no abort handler) is an unrecoverable error, not a revert.
    let exec_result = handler.try_commit_transaction();
    assert_eq!(exec_result, ExecResult::ExecResultUnrecoverable);
}

#[test]
fn test_mock_handler_panic_then_abort() {
    let env = TestEnvironment::new(None);
    let mut handler = MockTransactionHandler::new(env);

    handler
        .open_root_frame()
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
        tx.call(&[1u8; 32].into()).run(|| {
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
    assert_eq!(exec_result, ExecResult::ExecResultOK);
}

#[test]
fn test_transact_manual_abort() {
    let env = TestEnvironment::new(None);

    env.transact_manual(|tx| {
        let frame_result = tx.call(&[1u8; 32].into()).run(|| {
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
            ExecResult::ExecResultTxRootFrameReverted
        };
        (frame_result, exec_result)
    });

    frame_result.assert_success();
    assert_eq!(exec_result, ExecResult::ExecResultOK);
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

    let frame_result = tx.call(&[1u8; 32].into()).run(|| {
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
        tx.call(&[1u8; 32].into()).run(|| {
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
        tx.call(&[1u8; 32].into()).run(|| {
            // Trigger an abort via the Return syscall with revert_frame=true
            groundhog_xdr::syscalls::raw_Return(&[42u8, 43, 44], true);
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
    result.assert_event(0, |e| {
        assert_eq!(e.num_topics, 0);
        assert_eq!(&e.serialized_event[..], b"hello");
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
    result.assert_event(0, |e| {
        assert_eq!(&e.serialized_event[..], b"first");
    });
    result.assert_event(1, |e| {
        assert_eq!(&e.serialized_event[..], b"second");
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
    result.assert_event(0, |e| {
        assert_eq!(e.num_topics, 1);
        // First 32 bytes are the topic, rest is the data
        assert_eq!(&e.serialized_event[..32], topic.as_ref());
        assert_eq!(&e.serialized_event[32..], b"data");
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
    result.assert_event(0, |e| {
        assert_eq!(e.contract_address, addr);
    });
}

#[test]
fn test_frame_events_empty_on_fault() {
    let env = TestEnvironment::new(None);

    let result = env.transact(|tx| {
        tx.call(&[1u8; 32].into()).run(|| {
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
        tx.call(&[1u8; 32].into()).run(|| {
            groundhog_xdr::syscalls::raw_LogEvent(
                None,
                None,
                None,
                b"before_revert",
            )
            .unwrap();
            groundhog_xdr::syscalls::raw_Return(&[1], true);
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
    frame_result.assert_event(0, |e| {
        assert_eq!(&e.serialized_event[..], b"manual");
    });
}

// ── Typed event assertion tests ───────────────────────────────────────

#[test]
fn test_assert_event_data_u32() {
    let env = TestEnvironment::new(None);

    let result = env.transact(|tx| {
        tx.call(&[1u8; 32].into()).run(|| {
            let v: u32 = 12345;
            let data = xdrs::txs::encode(&v).unwrap();
            groundhog_xdr::syscalls::raw_LogEvent(None, None, None, &data)
                .unwrap();
        })
    });

    result.assert_success();
    assert_eq!(result.events().len(), 1);

    // Decode the event payload as u32 and check
    result.assert_event_data::<u32>(0, |v| {
        assert_eq!(*v, 12345);
    });

    // Also works on the frame directly
    result.frame().assert_event_data::<u32>(0, |v| {
        assert_eq!(*v, 12345);
    });
}

#[test]
fn test_assert_event_data_with_topic() {
    let env = TestEnvironment::new(None);

    let result = env.transact(|tx| {
        tx.call(&[1u8; 32].into()).run(|| {
            let v: u32 = 99;
            let data = xdrs::txs::encode(&v).unwrap();
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
    // assert_event_data strips topics and decodes payload
    result.assert_event_data::<u32>(0, |v| {
        assert_eq!(*v, 99);
    });
}

#[test]
fn test_assert_emitted_finds_matching_event() {
    let env = TestEnvironment::new(None);

    let result = env.transact(|tx| {
        tx.call(&[1u8; 32].into()).run(|| {
            for v in [10u32, 20, 30] {
                let data = xdrs::txs::encode(&v).unwrap();
                groundhog_xdr::syscalls::raw_LogEvent(None, None, None, &data)
                    .unwrap();
            }
        })
    });

    result.assert_success();
    assert_eq!(result.events().len(), 3);

    // Find the event with value 20
    result.assert_emitted::<u32>(|v| *v == 20);

    // Also works on the frame directly
    result.frame().assert_emitted::<u32>(|v| *v == 30);
}

#[test]
#[should_panic(expected = "no event matched the predicate")]
fn test_assert_emitted_panics_when_no_match() {
    let env = TestEnvironment::new(None);

    let result = env.transact(|tx| {
        tx.call(&[1u8; 32].into()).run(|| {
            let data = xdrs::txs::encode(&42u32).unwrap();
            groundhog_xdr::syscalls::raw_LogEvent(None, None, None, &data)
                .unwrap();
        })
    });

    result.assert_success();
    result.assert_emitted::<u32>(|v| *v == 999);
}

#[test]
#[should_panic(expected = "no event matched the predicate: 0 of 0")]
fn test_assert_emitted_panics_on_empty_events() {
    let env = TestEnvironment::new(None);

    let result = env.transact(|tx| tx.call(&[1u8; 32].into()).run(|| {}));

    result.assert_success();
    result.assert_emitted::<u32>(|_| true);
}

#[test]
#[should_panic(expected = "event index 5 out of range")]
fn test_assert_event_data_out_of_range() {
    let env = TestEnvironment::new(None);

    let result = env.transact(|tx| {
        tx.call(&[1u8; 32].into()).run(|| {
            let data = xdrs::txs::encode(&1u32).unwrap();
            groundhog_xdr::syscalls::raw_LogEvent(None, None, None, &data)
                .unwrap();
        })
    });

    result.assert_success();
    result.assert_event_data::<u32>(5, |_| {});
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
        tx.call(&[1u8; 32].into()).run(|| {
            let v: u32 = 42;
            let data = xdrs::txs::encode(&v).unwrap();
            groundhog_xdr::syscalls::raw_Return(&data, false);
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
                let data = xdrs::txs::encode(&v).unwrap();
                groundhog_xdr::syscalls::raw_LogEvent(None, None, None, &data)
                    .unwrap();
            }
        })
    });

    result.assert_success();
    let values: Vec<u32> = result.filter_events();
    assert_eq!(values, vec![10, 20, 30]);
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

// ── Gas limit regression test ─────────────────────────────────────────

#[test]
fn test_mock_handler_syscall_sha256() {
    let env = TestEnvironment::new(None);
    let mut handler = MockTransactionHandler::new(env);

    let result = handler
        .open_root_frame()
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
