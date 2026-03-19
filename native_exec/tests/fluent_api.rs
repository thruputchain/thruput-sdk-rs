use groundhog_native_exec::frame_result::FrameResult;
use groundhog_native_exec::transact_result::TransactResult;
use groundhog_xdr::exec_result::ExecResult;
use groundhog_xdr::transaction::InvocationFrameOutput;

#[test]
fn test_frame_result_success() {
    let return_data = vec![1u8, 2, 3];
    let output = InvocationFrameOutput::kInvocationFrameStatusCodeReturn {
        return_buffer: return_data.clone().try_into().unwrap(),
    };
    let result = FrameResult::new(output, None);

    assert!(result.is_success());
    assert!(!result.is_abort());
    assert!(!result.is_fault());
    result.assert_success();
    assert_eq!(result.return_buffer(), &[1, 2, 3]);
}

#[test]
fn test_frame_result_abort() {
    let abort_data = vec![4u8, 5, 6];
    let output = InvocationFrameOutput::kInvocationFrameStatusCodeAbort {
        abort_buffer: abort_data.clone().try_into().unwrap(),
    };
    let result = FrameResult::new(output, None);

    assert!(result.is_abort());
    assert!(!result.is_success());
    result.assert_abort();
    assert_eq!(result.abort_buffer(), &[4, 5, 6]);
}

#[test]
fn test_frame_result_fault() {
    let output = InvocationFrameOutput::kInvocationFrameStatusCodeFault;
    let result = FrameResult::new(output, None);

    assert!(result.is_fault());
    result.assert_fault();
}

#[test]
#[should_panic(expected = "expected success")]
fn test_frame_result_assert_success_panics_on_fault() {
    let output = InvocationFrameOutput::kInvocationFrameStatusCodeFault;
    let result = FrameResult::new(output, None);
    result.assert_success();
}

#[test]
#[should_panic(expected = "expected abort")]
fn test_frame_result_assert_abort_panics_on_success() {
    let return_data = vec![];
    let output = InvocationFrameOutput::kInvocationFrameStatusCodeReturn {
        return_buffer: return_data.try_into().unwrap(),
    };
    let result = FrameResult::new(output, None);
    result.assert_abort();
}

#[test]
fn test_transact_result_success() {
    let return_data = vec![];
    let output = InvocationFrameOutput::kInvocationFrameStatusCodeReturn {
        return_buffer: return_data.try_into().unwrap(),
    };
    let frame = FrameResult::new(output, None);
    let result = TransactResult::new(frame, ExecResult::ExecResultOK, None);
    result.assert_success();
}

#[test]
fn test_transact_result_abort() {
    let abort_data = vec![];
    let output = InvocationFrameOutput::kInvocationFrameStatusCodeAbort {
        abort_buffer: abort_data.try_into().unwrap(),
    };
    let frame = FrameResult::new(output, None);
    let result = TransactResult::new(
        frame,
        ExecResult::ExecResultTxRootFrameReverted,
        None,
    );
    result.assert_abort();
}

#[test]
fn test_transact_result_commit_failed() {
    let return_data = vec![];
    let output = InvocationFrameOutput::kInvocationFrameStatusCodeReturn {
        return_buffer: return_data.try_into().unwrap(),
    };
    let frame = FrameResult::new(output, None);
    let result = TransactResult::new(
        frame,
        ExecResult::ExecResultTxDeterministicError,
        None,
    );
    result.assert_commit_failed();
}

#[test]
#[should_panic(expected = "expected success")]
fn test_transact_result_assert_success_panics_on_abort() {
    let abort_data = vec![];
    let output = InvocationFrameOutput::kInvocationFrameStatusCodeAbort {
        abort_buffer: abort_data.try_into().unwrap(),
    };
    let frame = FrameResult::new(output, None);
    let result = TransactResult::new(
        frame,
        ExecResult::ExecResultTxRootFrameReverted,
        None,
    );
    result.assert_success();
}

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
        .execute(|_| false)
        .result();

    handler.notify_root_frame_closed();
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
        .execute(|_| false)
        .result();

    handler.notify_root_frame_closed();
    handler.abort_transaction();
}

#[test]
fn test_mock_handler_event_logs_empty() {
    let env = TestEnvironment::new(None);
    let mut handler = MockTransactionHandler::new(env);

    handler
        .open_root_frame()
        .with_invoke_flags(0)
        .build()
        .execute(|_| false)
        .result();

    handler.notify_root_frame_closed();
    handler.try_commit_transaction();

    // No events emitted, should return None or empty
    let events = handler.get_event_logs();
    assert!(events.is_none() || events.unwrap().events.is_empty());
}

use groundhog_native_exec::testing::TestEnvExt;
use groundhog_native_exec::transaction::Transaction;

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
        tx.commit()
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
        tx.abort();
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
            tx.commit()
        } else {
            tx.abort();
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
    let mut tx = Transaction::new(env);

    let frame_result = tx
        .call(&[1u8; 32].into())
        .run(|| {
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
    let mut tx = Transaction::new(env);

    let frame_result = tx
        .call(&[1u8; 32].into())
        .run(|| {
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

    let result = env.transact(|tx| {
        tx.call(&[1u8; 32].into()).run(|| {})
    });

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

use groundhog_native_exec::frame_action::FrameAction;

#[test]
fn test_run_continuable_basic() {
    let env = TestEnvironment::new(None);
    let mut tx = Transaction::new(env);

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
    let mut tx = Transaction::new(env);

    let result = tx
        .call(&[1u8; 32].into())
        .run_continuable(|frame| {
            // Open a child frame from within the continuable frame
            let child_result = frame
                .child_call(&[2u8; 32].into())
                .run(|| {
                    let addr = groundhog_xdr::syscalls::GetCurrentAddress().unwrap();
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
