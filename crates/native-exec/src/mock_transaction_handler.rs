use crate::frame_builder::FrameBuilder;
use crate::frame_result::FrameResult;
use crate::invoke_error::InvokeError;
use crate::test_environment::TestEnvironment;
use core::ffi::c_void;
use groundhog_sys::ForeignTryCommitResult;
use groundhog_trace::InvocationFrameTrace;
use groundhog_xdr::auth_claim::AuthClaim;
use groundhog_xdr::event::TxEmittedEvent;
use groundhog_xdr::transaction::InvocationFrameOutput;
use groundhog_xdr::types::SHA256Hash;
use std::cell::Cell;
use std::cell::RefCell;
use std::panic::UnwindSafe;
use std::panic::panic_any;
use std::rc::Rc;

use groundhog_sys::FFIErrorCode;

#[derive(Debug, PartialEq)]
pub(crate) enum MockTransactionState {
    NotStarted,
    Running,
    Aborted,
    Committed,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, PartialEq)]
enum MockInvocationFrameState {
    Started,
    Closed(
        InvocationFrameOutput,
        Option<InvocationFrameTrace>,
        Vec<TxEmittedEvent>,
    ),
}

pub struct MockInvocationFrameConfig<'a> {
    context_pushcount: Rc<RefCell<ThreadlocalContextPushcount>>,
    invocation: groundhog_xdr::transaction::Invocation,
    invoke_flags: i32,

    root_handler: *mut c_void,
    // when we build the frame, the new frame will be emplaced into this value
    to_emplace: &'a mut Option<Box<MockInvocationFrame>>,
    root_frame_closed: Option<Rc<Cell<bool>>>,
}

impl<'a> MockInvocationFrameConfig<'a> {
    fn new(
        root_handler: *mut c_void,
        to_emplace: &'a mut Option<Box<MockInvocationFrame>>,
        context_pushcount: Rc<RefCell<ThreadlocalContextPushcount>>,
        root_frame_closed: Option<Rc<Cell<bool>>>,
    ) -> Self {
        Self {
            context_pushcount,
            root_handler,
            invocation: groundhog_xdr::transaction::Invocation {
                invoked_address: [0; 32].into(),
                entrypoint: [0; 8].into(),
                gas_limit: u64::MAX,
                calldata: xdr_common::BytesM::default(),
            },
            invoke_flags: 0,
            to_emplace,
            root_frame_closed,
        }
    }

    pub fn at_contract_address(
        mut self,
        contract_address: groundhog_xdr::types::ContractAddress,
    ) -> Self {
        self.invocation.invoked_address = contract_address;
        self
    }

    pub fn with_calldata(mut self, calldata: xdr_common::BytesM) -> Self {
        self.invocation.calldata = calldata;
        self
    }

    pub fn with_invoke_flags(mut self, invoke_flags: i32) -> Self {
        self.invoke_flags = invoke_flags;
        self
    }

    pub fn with_gas_limit(mut self, gas_limit: u64) -> Self {
        self.invocation.gas_limit = gas_limit;
        self
    }

    pub fn build(self) -> &'a mut Box<MockInvocationFrame> {
        // Frame-open bookkeeping (invocation encode, frame Box, FFI
        // open) is harness work, not the enclosing frame's contract
        // code.
        let _alloc_pause = crate::alloc_stats::PauseGuard::new();
        let serialized_invocation: Vec<u8> =
            xdrs::txs::encode(&self.invocation).unwrap();
        let frame = Box::new(MockInvocationFrame::new(
            self.root_handler,
            &serialized_invocation,
            self.invoke_flags,
            self.context_pushcount.clone(),
            self.root_frame_closed,
        ));
        *self.to_emplace = Some(frame);
        self.to_emplace.as_mut().unwrap()
    }
}

pub struct MockInvocationFrame {
    root_handler: *mut c_void,
    child_frame: Option<Box<MockInvocationFrame>>,
    state: MockInvocationFrameState,
    context_pushcount: Rc<RefCell<ThreadlocalContextPushcount>>,
    root_frame_closed: Option<Rc<Cell<bool>>>,
    // Exclusive allocation stats accumulated across this frame's
    // `execute` steps (a frame spans several via FrameAction::Continue),
    // plus the cross-call records claimed from those windows.
    alloc_stats: crate::alloc_stats::AllocStats,
    cross_call_alloc_records: Vec<crate::alloc_stats::FrameAllocRecord>,
}

impl MockInvocationFrame {
    fn new(
        root_handler: *mut c_void,
        serialized_invocation: &[u8],
        invoke_flags: i32,
        context_pushcount: Rc<RefCell<ThreadlocalContextPushcount>>,
        root_frame_closed: Option<Rc<Cell<bool>>>,
    ) -> Self {
        let ffi_invocation = groundhog_sys::FFISerializedInvocation {
            data: serialized_invocation.as_ptr() as *mut u8,
            len: serialized_invocation.len().try_into().unwrap(),
        };

        let handler_res: groundhog_sys::FFIErrorCode = unsafe {
            groundhog_sys::open_mock_invocation_frame(
                root_handler,
                ffi_invocation,
                invoke_flags,
            )
            .into()
        };
        if handler_res != FFIErrorCode::None {
            eprintln!(
                "failed to open mock invocation frame: {:?}",
                handler_res
            );
            panic_any(crate::invoke_error::unrecoverable_error());
        }

        Self {
            root_handler,
            child_frame: None,
            state: MockInvocationFrameState::Started,
            context_pushcount,
            root_frame_closed,
            alloc_stats: crate::alloc_stats::AllocStats::ZERO,
            cross_call_alloc_records: Vec::new(),
        }
    }

    pub fn open_child_frame(&mut self) -> MockInvocationFrameConfig<'_> {
        let config = MockInvocationFrameConfig::new(
            self.root_handler,
            &mut self.child_frame,
            self.context_pushcount.clone(),
            None,
        );
        config
    }

    fn close_frame_internal(
        &mut self,
        error_code: hogexec_internal::ffi_enums::InvokeErrorCode,
    ) {
        let handler_res = unsafe {
            groundhog_sys::finish_mock_invocation_frame(
                self.root_handler,
                error_code.0,
            )
        };
        let err: FFIErrorCode = handler_res.ffi_error_code.into();
        if err != FFIErrorCode::None {
            eprintln!("failed to close mock invocation frame: {:?}", err);
            panic_any(crate::invoke_error::unrecoverable_error());
        }

        let output: InvocationFrameOutput = xdrs::txs::decode(
            handler_res.serialized_output.as_slice().unwrap(),
        )
        .unwrap();
        let trace: Option<InvocationFrameTrace> = if let Some(trace_data) =
            handler_res.serialized_trace.as_slice()
        {
            let deserialized_trace: InvocationFrameTrace =
                xdrs::txs::decode(trace_data).unwrap();
            Some(deserialized_trace)
        } else {
            None
        };
        let events: Vec<TxEmittedEvent> = if let Some(events_data) =
            handler_res.serialized_events.as_slice()
        {
            let decoded: xdr_common::VecM<TxEmittedEvent> =
                xdrs::txs::decode(events_data).unwrap_or_else(|e| {
                    panic!(
                        "failed to decode frame events ({} bytes): {}",
                        events_data.len(),
                        e
                    )
                });
            decoded.into()
        } else {
            Vec::new()
        };
        self.state = MockInvocationFrameState::Closed(output, trace, events);
        if let Some(flag) = &self.root_frame_closed {
            flag.set(true);
        }
    }

    fn push_context(&mut self) {
        self.context_pushcount
            .borrow_mut()
            .push_context(self.root_handler);
    }

    fn pop_context(&mut self) {
        self.context_pushcount.borrow_mut().pop_context();
    }

    pub fn execute(
        &mut self,
        lambda: impl FnOnce(&mut Self) -> crate::frame_builder::FrameAction
        + UnwindSafe,
    ) -> &mut Self {
        use crate::frame_builder::FrameAction;

        if self.state != MockInvocationFrameState::Started {
            eprintln!(
                "Should be impossible to execute a frame while it is not started"
            );
            panic_any(crate::invoke_error::unrecoverable_error());
        }

        self.push_context();
        crate::invoke_error::activate_panic_hook();

        let records_start = crate::alloc_stats::records_len();
        let alloc_scope = crate::alloc_stats::FrameMeasureScope::open();
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                lambda(self)
            }));
        // Closing after catch_unwind covers panic/abort paths too. The
        // scope reports stats exclusive of nested frames (child mock
        // frames and trampoline cross-calls both open their own scopes),
        // matching the real system where each frame is a separate guest
        // instance with its own heap. Cross-call records logged during
        // this window are claimed onto this frame; explicit child
        // frames claimed theirs before this window closes.
        let step_stats = alloc_scope.close();
        // The rest of execute is frame-teardown bookkeeping (record
        // claiming, context pop, FFI frame close + output/trace/event
        // decode); pause so nested frames don't charge it to their
        // parent's still-open window.
        let _alloc_pause = crate::alloc_stats::PauseGuard::new();
        self.alloc_stats = self.alloc_stats + step_stats;
        self.cross_call_alloc_records
            .extend(crate::alloc_stats::take_records_from(records_start));

        crate::invoke_error::deactivate_panic_hook();
        self.pop_context();

        match result {
            Ok(FrameAction::Close) => {
                self.close_frame_internal(
                    hogexec_internal::ffi_enums::InvokeErrorCode::None,
                );
            }
            Ok(FrameAction::Continue) => {}
            Err(err) => {
                if let Some(my_error) = err.downcast_ref::<InvokeError>() {
                    self.close_frame_internal(my_error.error_type);
                } else {
                    eprintln!("unrecoverable error: {:?}", err);
                    self.close_frame_internal(hogexec_internal::ffi_enums::InvokeErrorCode::Unrecoverable);
                }
            }
        };
        self
    }

    pub fn result(&mut self) -> FrameResult {
        // FrameResult construction clones output/trace/events — harness
        // bookkeeping, possibly inside a parent frame's open window.
        let _alloc_pause = crate::alloc_stats::PauseGuard::new();
        let res = match &self.state {
            MockInvocationFrameState::Closed(output, trace, events) => {
                FrameResult::new(output.clone(), trace.clone(), events.clone())
                    .with_alloc_measurements(
                        self.alloc_stats,
                        self.cross_call_alloc_records.clone(),
                    )
            }
            _ => {
                eprintln!(
                    "Cannot get the result of a frame while it is not closed"
                );
                panic_any(crate::invoke_error::unrecoverable_error());
            }
        };
        self.child_frame = None;
        res
    }
}

impl Drop for MockInvocationFrame {
    fn drop(&mut self) {
        // Close any open child frame first — the FFI requires inner frames be
        // finished before their parent. (The `child_frame` field would
        // otherwise drop after this body.)
        self.child_frame = None;

        if self.state == MockInvocationFrameState::Started {
            // The frame's FFI handle is still open: a test almost certainly
            // panicked (or returned early) before driving it to Close. Finish
            // it at the FFI level so the frame's coroutine completes and the
            // handler can be torn down cleanly. Leaving it open orphans the
            // coroutine, which later wedges the libtest_ffi global
            // `CoroPoll::~CoroPoll` -> `DestroyInhibit::destroy_wait()` and
            // hangs the process at exit. We must NOT panic here: we may already
            // be unwinding from the original failure, and a second panic would
            // abort with a SIGABRT that hides it. Force-finishing is the
            // recoverable cleanup the old `panic!` never performed.
            let res = unsafe {
                groundhog_sys::finish_mock_invocation_frame(
                    self.root_handler,
                    hogexec_internal::ffi_enums::InvokeErrorCode::Unrecoverable
                        .0,
                )
            };
            // An `UnrecoverableError` status is expected here — we finished an
            // abandoned frame with the Unrecoverable code; it still completes
            // the coroutine. Log only so an unexpected status isn't silent.
            let err: FFIErrorCode = res.ffi_error_code.into();
            if err != FFIErrorCode::None
                && err != FFIErrorCode::UnrecoverableError
            {
                eprintln!(
                    "unexpected status finishing an abandoned invocation frame \
                     during cleanup: {:?}",
                    err
                );
            }
            if let Some(flag) = &self.root_frame_closed {
                flag.set(true);
            }
        }
    }
}

pub(crate) struct ThreadlocalContextPushcount {
    count: i32,
}

impl ThreadlocalContextPushcount {
    pub fn new() -> Self {
        Self { count: 0 }
    }

    pub fn push_context(&mut self, handler: *mut c_void) {
        if self.count == 0 {
            let push_res = unsafe { groundhog_sys::push_context(handler) };
            let err: FFIErrorCode = push_res.into();
            if err != FFIErrorCode::None {
                panic!("failed to push context: {:?}", err);
            }
        }
        self.count += 1;
    }

    pub fn pop_context(&mut self) {
        if self.count == 0 {
            panic!("cannot pop context if count is 0");
        }
        self.count -= 1;
        if self.count == 0 {
            let pop_res = unsafe { groundhog_sys::pop_context() };
            let err: FFIErrorCode = pop_res.into();
            if err != FFIErrorCode::None {
                panic!("failed to pop context: {:?}", err);
            }
        }
    }
}

impl Drop for ThreadlocalContextPushcount {
    fn drop(&mut self) {
        if self.count != 0 {
            eprintln!(
                "Dropping transaction while threadlocal context active: count={}",
                self.count
            );
            let pop_res = unsafe { groundhog_sys::pop_context() };
            let err: FFIErrorCode = pop_res.into();
            if err != FFIErrorCode::None {
                crate::drop_failure(format_args!(
                    "failed to pop context: {:?}",
                    err
                ));
            }
        }
    }
}

// Can run one, and only one, transaction,
// and then get the results
pub struct MockTransactionHandler {
    pub(crate) handler: *mut c_void,
    pub(crate) _env: Rc<TestEnvironment>,
    pub(crate) state: MockTransactionState,
    pub(crate) root_frame: Option<Box<MockInvocationFrame>>,
    pub(crate) context_pushcount: Rc<RefCell<ThreadlocalContextPushcount>>,
    root_frame_closed: Rc<Cell<bool>>,
}

impl MockTransactionHandler {
    pub fn new(env: Rc<TestEnvironment>) -> Self {
        // TODO(geoff): ways to mock this out
        use groundhog_xdr::auth_claim::AuthClaim_source_address_claim;
        let auth_claim: AuthClaim = AuthClaim::kSourceAddressAuthData {
            source_address_claim: AuthClaim_source_address_claim {
                allowed_keys: 0,
                required_weight: 0,
            },
        };

        let serialized_auth_claim = xdrs::txs::encode(&auth_claim).unwrap();
        let source_tx_hash: SHA256Hash = [0; 32].into();
        let handler_res = unsafe {
            groundhog_sys::open_mocked_syscall_handler(
                env.inner.borrow().db.db,
                env.inner.borrow().contract_context.context,
                groundhog_sys::FFISerializedAuthClaimData {
                    data: serialized_auth_claim.as_ptr() as *mut u8,
                    len: serialized_auth_claim.len().try_into().unwrap(),
                },
                source_tx_hash.as_ptr(),
                /*enable_tracing*/ 1,
            )
        };
        let err: FFIErrorCode = handler_res.ffi_error_code.into();
        if err != FFIErrorCode::None {
            panic!("failed to open mocked syscall handler: {:?}", err);
        }
        assert!(!handler_res.syscall_handler.is_null());
        Self {
            handler: handler_res.syscall_handler,
            _env: env,
            state: MockTransactionState::NotStarted,
            root_frame: None,
            context_pushcount: Rc::new(RefCell::new(
                ThreadlocalContextPushcount::new(),
            )),
            root_frame_closed: Rc::new(Cell::new(false)),
        }
    }

    pub fn abort_transaction(&mut self) {
        assert_eq!(self.state, MockTransactionState::Running);
        assert!(
            self.root_frame_closed.get(),
            "cannot abort: root frame is still open"
        );
        self.state = MockTransactionState::Aborted;
        let err: FFIErrorCode =
            unsafe { groundhog_sys::abort_transaction(self.handler) }.into();
        if err != FFIErrorCode::None {
            panic!("failed to abort transaction: {:?}", err);
        }
    }

    pub fn try_commit_transaction(
        &mut self,
    ) -> groundhog_xdr::exec_result::ExecResult {
        assert_eq!(self.state, MockTransactionState::Running);
        assert!(
            self.root_frame_closed.get(),
            "cannot commit: root frame is still open"
        );
        self.state = MockTransactionState::Committed;
        let result: ForeignTryCommitResult =
            unsafe { groundhog_sys::try_commit_transaction(self.handler) };
        let err: FFIErrorCode = result.ffi_error_code.into();
        if err != FFIErrorCode::None {
            panic!("ffi error: failed to commit transaction: {:?}", err);
        }

        let exec_result: groundhog_xdr::exec_result::ExecResult =
            result.result_code.into();
        exec_result
    }

    /// Convenience: open a root frame targeting the given contract address.
    /// Returns a FrameBuilder for configuring and executing the frame.
    pub fn call(
        &mut self,
        addr: &groundhog_xdr::types::ContractAddress,
    ) -> FrameBuilder<'_> {
        let config = self.open_root_frame().at_contract_address(*addr);
        FrameBuilder::new(config)
    }

    pub fn open_root_frame(&mut self) -> MockInvocationFrameConfig<'_> {
        assert_eq!(
            self.state,
            MockTransactionState::NotStarted,
            "cannot open root frame: transaction already started"
        );
        self.state = MockTransactionState::Running;
        let config = MockInvocationFrameConfig::new(
            self.handler,
            &mut self.root_frame,
            self.context_pushcount.clone(),
            Some(self.root_frame_closed.clone()),
        );
        config
    }
}

impl Drop for MockTransactionHandler {
    fn drop(&mut self) {
        // Tear down in the order the FFI requires: finish any still-open
        // invocation frame BEFORE aborting the transaction and closing the
        // handler. Field-drop order would otherwise run the frame's
        // `finish_mock_invocation_frame` dead-last — after `close_syscall_handler`
        // (handler body) and after `_env` drops `free_db`/`close_runtime_context`
        // — so every FFI call returns `UnrecoverableError` and the frame's
        // coroutine is left in-flight, wedging the libtest_ffi global
        // `CoroPoll::~CoroPoll` -> `DestroyInhibit::destroy_wait()` at process
        // exit. Dropping `root_frame` here finishes the frame while the handler
        // is still live.
        self.root_frame = None;

        if self.state == MockTransactionState::Running {
            // Abort the uncommitted transaction. Inlined rather than calling
            // `abort_transaction` to avoid its asserts: we may be unwinding from
            // a test failure, and a panic in `drop` would double-panic into a
            // SIGABRT that hides the original message.
            self.state = MockTransactionState::Aborted;
            let err: FFIErrorCode =
                unsafe { groundhog_sys::abort_transaction(self.handler) }
                    .into();
            if err != FFIErrorCode::None {
                crate::drop_failure(format_args!(
                    "failed to abort uncommitted transaction during cleanup: {:?}",
                    err
                ));
            }
        }

        let err: FFIErrorCode =
            unsafe { groundhog_sys::close_syscall_handler(self.handler) }
                .into();
        if err != FFIErrorCode::None {
            crate::drop_failure(format_args!(
                "failed to close syscall handler: {:?}",
                err
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use groundhog_xdr::transaction::InvocationFrameStatusCode;

    use crate::frame_builder::FrameAction;

    use super::*;

    #[test]
    fn test_mock_transaction_handler() {
        let env = TestEnvironment::new(None);
        let mut handler = MockTransactionHandler::new(env);

        static mut FIRST: bool = false;
        static mut SECOND: bool = false;

        let result = handler
            .open_root_frame()
            .with_invoke_flags(0)
            .build()
            .execute(|_| {
                unsafe {
                    FIRST = true;
                }
                FrameAction::Continue
            })
            .execute(|_| {
                unsafe {
                    SECOND = true;
                }
                FrameAction::Close
            })
            .result();

        assert!(unsafe { FIRST });
        assert!(unsafe { SECOND });
        assert_eq!(
            result.output().status_code(),
            InvocationFrameStatusCode::Return
        );
    }

    #[test]
    fn panic_with_open_transaction_is_not_masked_by_drop() {
        // Regression: a test that panics while a root frame is still open used
        // to abort the whole process. Originally the Drop impls double-panicked
        // (`MockInvocationFrame::drop` asserted the frame wasn't `Started`;
        // `MockTransactionHandler::drop`->`abort_transaction` asserted the root
        // frame was closed), turning a clean test failure into a SIGABRT that
        // hid the message. Once those panics were removed, a second bug showed:
        // the Drops tore the FFI down in the wrong order (handler/db freed
        // before the open frame was finished), orphaning the frame's coroutine
        // and hanging the process at exit in `CoroPoll::~CoroPoll`. The Drops
        // now finish the open frame first and never panic, so the original
        // panic propagates cleanly and the process exits.
        let caught =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let env = TestEnvironment::new(None);
                let mut handler = MockTransactionHandler::new(env);
                // Transaction -> Running, root frame -> Started, but never
                // closed or committed.
                handler.open_root_frame().with_invoke_flags(0).build();
                panic!("original failure message");
            }));

        let payload = caught.expect_err("the panic must propagate, not abort");
        let msg = payload
            .downcast_ref::<&'static str>()
            .copied()
            .or_else(|| payload.downcast_ref::<String>().map(String::as_str));
        assert_eq!(
            msg,
            Some("original failure message"),
            "the original panic message must survive Drop cleanup"
        );
    }

    #[test]
    fn test_noop_mock_transaction_handler() {
        let env = TestEnvironment::new(None);
        let _handler = MockTransactionHandler::new(env);
    }

    #[test]
    fn test_mock_transaction_handler_with_error() {
        let env = TestEnvironment::new(None);
        let mut handler = MockTransactionHandler::new(env);

        let result = handler
            .open_root_frame()
            .with_invoke_flags(0)
            .build()
            .execute(|_| {
                panic!("test error");
            })
            .result();
        assert_eq!(
            result.output().status_code(),
            InvocationFrameStatusCode::Fault
        );
    }

    #[test]
    fn test_nested_mocks() {
        let env = TestEnvironment::new(None);
        let mut handler = MockTransactionHandler::new(env);

        static mut FIRST: bool = false;
        static mut SECOND: bool = false;

        let result = handler
            .open_root_frame()
            .with_invoke_flags(0)
            .build()
            .execute(|frame| {
                frame
                    .open_child_frame()
                    .with_invoke_flags(0)
                    .build()
                    .execute(|_| {
                        unsafe {
                            assert!(!FIRST);
                            SECOND = true;
                        }
                        FrameAction::Close
                    })
                    .result();

                unsafe {
                    assert!(SECOND);
                    FIRST = true;
                }
                FrameAction::Close
            })
            .result();
        assert_eq!(
            result.output().status_code(),
            InvocationFrameStatusCode::Return
        );
        assert!(unsafe { FIRST });
        assert!(unsafe { SECOND });
    }

    #[test]
    fn test_syscalls_with_context() {
        let env = TestEnvironment::new(None);
        let mut handler = MockTransactionHandler::new(env);

        let result = handler
            .open_root_frame()
            .with_invoke_flags(0)
            .at_contract_address(groundhog_xdr::types::ContractAddress::from(
                [1; 32],
            ))
            .build()
            .execute(|_| {
                std::println!("getting current address");
                let res = groundhog_xdr::syscalls::GetCurrentAddress();
                std::println!("res: {:?}", res);
                assert_eq!(
                    res.unwrap(),
                    groundhog_xdr::types::ContractAddress::from([1; 32])
                );
                FrameAction::Close
            })
            .result();
        assert_eq!(
            result.output().status_code(),
            InvocationFrameStatusCode::Return
        );
    }

    #[test]
    fn test_nested_frames_keep_panic_hook_active() {
        let env = TestEnvironment::new(None);
        let mut handler = MockTransactionHandler::new(env);

        handler
            .open_root_frame()
            .with_invoke_flags(0)
            .build()
            .execute(|frame| {
                // Outer frame: hook should be active at depth 1
                assert_eq!(crate::invoke_error::panic_hook_depth(), 1);

                frame
                    .open_child_frame()
                    .with_invoke_flags(0)
                    .build()
                    // First execute: Continue, depth goes to 2 then back to 1
                    .execute(|_| {
                        assert_eq!(crate::invoke_error::panic_hook_depth(), 2);
                        FrameAction::Continue
                    })
                    // Second execute: still depth 2 inside, then Close
                    .execute(|_| {
                        assert_eq!(crate::invoke_error::panic_hook_depth(), 2);
                        FrameAction::Close
                    })
                    .result();

                // Back in outer frame after inner deactivate: still depth 1
                assert_eq!(crate::invoke_error::panic_hook_depth(), 1);
                assert!(crate::invoke_error::is_panic_hook_active());

                FrameAction::Close
            })
            .result();

        // After everything: depth back to 0
        assert_eq!(crate::invoke_error::panic_hook_depth(), 0);
        assert!(!crate::invoke_error::is_panic_hook_active());
    }

    #[test]
    fn test_nested_frame_panic_still_caught_by_outer() {
        let env = TestEnvironment::new(None);
        let mut handler = MockTransactionHandler::new(env);

        let result = handler
            .open_root_frame()
            .with_invoke_flags(0)
            .build()
            .execute(|frame| {
                // Child panics
                frame
                    .open_child_frame()
                    .with_invoke_flags(0)
                    .build()
                    .execute(|_| {
                        panic!("inner contract panic");
                    })
                    .result();

                // Outer frame should still be active and running
                assert!(crate::invoke_error::is_panic_hook_active());
                assert_eq!(crate::invoke_error::panic_hook_depth(), 1);

                FrameAction::Close
            })
            .result();

        assert_eq!(
            result.output().status_code(),
            InvocationFrameStatusCode::Return
        );
        assert_eq!(crate::invoke_error::panic_hook_depth(), 0);
    }

    #[test]
    fn test_interleaved_handlers_panic_hook_depth() {
        let env = TestEnvironment::new(None);
        let mut handler1 = MockTransactionHandler::new(env.clone());
        let mut handler2 = MockTransactionHandler::new(env);

        assert_eq!(crate::invoke_error::panic_hook_depth(), 0);

        // handler1: Continue with nested child frame
        let frame1 = handler1
            .open_root_frame()
            .with_invoke_flags(0)
            .build()
            .execute(|frame| {
                assert_eq!(crate::invoke_error::panic_hook_depth(), 1);

                frame
                    .open_child_frame()
                    .with_invoke_flags(0)
                    .build()
                    .execute(|_| {
                        assert_eq!(crate::invoke_error::panic_hook_depth(), 2);
                        FrameAction::Close
                    })
                    .result();

                assert_eq!(crate::invoke_error::panic_hook_depth(), 1);
                FrameAction::Continue
            });
        assert_eq!(crate::invoke_error::panic_hook_depth(), 0);

        // handler2: Continue (no children)
        let frame2 = handler2
            .open_root_frame()
            .with_invoke_flags(0)
            .build()
            .execute(|_| {
                assert_eq!(crate::invoke_error::panic_hook_depth(), 1);
                FrameAction::Continue
            });
        assert_eq!(crate::invoke_error::panic_hook_depth(), 0);

        // handler1: Close with nested child that panics
        let result1 = frame1
            .execute(|frame| {
                assert_eq!(crate::invoke_error::panic_hook_depth(), 1);

                frame
                    .open_child_frame()
                    .with_invoke_flags(0)
                    .build()
                    .execute(|_| {
                        assert_eq!(crate::invoke_error::panic_hook_depth(), 2);
                        panic!("child panic in handler1");
                    })
                    .result();

                // Child panicked but depth is back to 1
                assert_eq!(crate::invoke_error::panic_hook_depth(), 1);
                FrameAction::Close
            })
            .result();
        assert_eq!(crate::invoke_error::panic_hook_depth(), 0);

        // handler2: Close with nested child doing Continue then Close
        let result2 = frame2
            .execute(|frame| {
                assert_eq!(crate::invoke_error::panic_hook_depth(), 1);

                frame
                    .open_child_frame()
                    .with_invoke_flags(0)
                    .build()
                    .execute(|_| {
                        assert_eq!(crate::invoke_error::panic_hook_depth(), 2);
                        FrameAction::Continue
                    })
                    .execute(|_| {
                        assert_eq!(crate::invoke_error::panic_hook_depth(), 2);
                        FrameAction::Close
                    })
                    .result();

                assert_eq!(crate::invoke_error::panic_hook_depth(), 1);
                FrameAction::Close
            })
            .result();
        assert_eq!(crate::invoke_error::panic_hook_depth(), 0);

        assert_eq!(
            result1.output().status_code(),
            InvocationFrameStatusCode::Return
        );
        assert_eq!(
            result2.output().status_code(),
            InvocationFrameStatusCode::Return
        );
    }

    #[test]
    fn test_concurrent_transactions() {
        let env = TestEnvironment::new(None);
        let mut handler1 = MockTransactionHandler::new(env.clone());
        let mut handler2 = MockTransactionHandler::new(env);

        let frame1 = handler1
            .open_root_frame()
            .with_invoke_flags(0)
            .at_contract_address(groundhog_xdr::types::ContractAddress::from(
                [1; 32],
            ))
            .build()
            .execute(|_| {
                std::println!("getting current address");
                let res = groundhog_xdr::syscalls::GetCurrentAddress();
                std::println!("res: {:?}", res);
                assert_eq!(
                    res.unwrap(),
                    groundhog_xdr::types::ContractAddress::from([1; 32])
                );
                FrameAction::Continue
            });

        let frame2 = handler2
            .open_root_frame()
            .with_invoke_flags(0)
            .at_contract_address(groundhog_xdr::types::ContractAddress::from(
                [2; 32],
            ))
            .build()
            .execute(|_| {
                std::println!("getting current address");
                let res = groundhog_xdr::syscalls::GetCurrentAddress();
                std::println!("res: {:?}", res);
                assert_eq!(
                    res.unwrap(),
                    groundhog_xdr::types::ContractAddress::from([2; 32])
                );
                FrameAction::Continue
            });
        let result1 = frame1
            .execute(|_| {
                std::println!("getting current address");
                let res = groundhog_xdr::syscalls::GetCurrentAddress();
                std::println!("res: {:?}", res);
                assert_eq!(
                    res.unwrap(),
                    groundhog_xdr::types::ContractAddress::from([1; 32])
                );
                FrameAction::Close
            })
            .result();

        let result2 = frame2
            .execute(|_| {
                std::println!("getting current address");
                let res = groundhog_xdr::syscalls::GetCurrentAddress();
                std::println!("res: {:?}", res);
                assert_eq!(
                    res.unwrap(),
                    groundhog_xdr::types::ContractAddress::from([2; 32])
                );
                FrameAction::Close
            })
            .result();

        assert_eq!(
            result1.output().status_code(),
            InvocationFrameStatusCode::Return
        );
        assert_eq!(
            result2.output().status_code(),
            InvocationFrameStatusCode::Return
        );
    }
}
