use crate::frame_builder::FrameBuilder;
use crate::frame_result::FrameResult;
use crate::invoke_error::InvokeError;
use crate::test_environment::TestEnvironment;
use core::ffi::c_void;
use groundhog_sys::ForeignTryCommitResult;
use groundhog_trace::InvocationFrameTrace;
use groundhog_xdr::auth_claim::AuthClaim;
use groundhog_xdr::event::TxEmittedEvent;
use groundhog_xdr::syscall_error::SyscallErrorCode;
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

/// Builder for one invocation frame's configuration, opened with
/// [`build`](Self::build).
pub struct MockInvocationFrameConfig<'a> {
    context_pushcount: Rc<RefCell<ThreadlocalContextPushcount>>,
    invocation: groundhog_xdr::transaction::Invocation,
    invoke_flags: groundhog_xdr::invoke_flag::InvokeFlagSet,

    root_handler: *mut c_void,
    // when we build the frame, the new frame will be emplaced into this value
    to_emplace: &'a mut Option<Box<MockInvocationFrame>>,
    root_frame_closed: Option<Rc<Cell<bool>>>,
}

impl<'a> MockInvocationFrameConfig<'a> {
    // The invoked address is a constructor parameter rather than an optional
    // builder step: the test FFI admits mock frames under production's
    // reentrance rule, which refuses an address that already has an active
    // frame (hogd #3301). A default would make "forgot to set it" the
    // reentrant case, and only at run time.
    fn new(
        root_handler: *mut c_void,
        contract_address: groundhog_xdr::types::ContractAddress,
        to_emplace: &'a mut Option<Box<MockInvocationFrame>>,
        context_pushcount: Rc<RefCell<ThreadlocalContextPushcount>>,
        root_frame_closed: Option<Rc<Cell<bool>>>,
    ) -> Self {
        Self {
            context_pushcount,
            root_handler,
            invocation: groundhog_xdr::transaction::Invocation {
                invoked_address: contract_address,
                entrypoint: [0; 8].into(),
                gas_limit: u64::MAX,
                calldata: xdr_common::BytesM::default(),
            },
            invoke_flags: 0,
            to_emplace,
            root_frame_closed,
        }
    }

    /// Set the calldata the invoked frame observes (default: empty).
    pub fn with_calldata(mut self, calldata: xdr_common::BytesM) -> Self {
        self.invocation.calldata = calldata;
        self
    }

    /// Set the frame's invoke flags, a bitset of
    /// [`InvokeFlag`](groundhog_xdr::invoke_flag::InvokeFlag) values
    /// (default: none).
    pub fn with_invoke_flags(
        mut self,
        invoke_flags: groundhog_xdr::invoke_flag::InvokeFlagSet,
    ) -> Self {
        self.invoke_flags = invoke_flags;
        self
    }

    /// Set the frame's gas limit (default: `u64::MAX`).
    pub fn with_gas_limit(mut self, gas_limit: u64) -> Self {
        self.invocation.gas_limit = gas_limit;
        self
    }

    /// Open the frame at the FFI level, returning the
    /// [`MockInvocationFrame`].
    pub fn build(self) -> &'a mut Box<MockInvocationFrame> {
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

/// One open invocation frame in a mocked transaction. Drive it with
/// [`execute`](Self::execute), then read the outcome with
/// [`result`](Self::result). Prefer the fluent
/// [`crate::frame_builder::FrameBuilder`] API unless a test needs this
/// level of control.
pub struct MockInvocationFrame {
    root_handler: *mut c_void,
    child_frame: Option<Box<MockInvocationFrame>>,
    state: MockInvocationFrameState,
    context_pushcount: Rc<RefCell<ThreadlocalContextPushcount>>,
    root_frame_closed: Option<Rc<Cell<bool>>>,
    /// The panic that faulted THIS frame's body, recorded at `execute`'s
    /// catch site.
    ///
    /// Set only for a plain panic out of this frame's own lambda, which is
    /// what makes it this frame's rather than a nested one's: a child frame
    /// catches its own panic and closes itself, so the panic never reaches
    /// here; and a `raw_Invoke`ed callee is caught by `invoke_wrapper`,
    /// which has no frame to record against at all (see `fault_panic`).
    fault_panic: Option<crate::invoke_error::PanicRecord>,
}

/// Name the rule that refused a frame, rather than leaving a caller with a
/// bare code. These are production's own refusals: a contract's `Invoke` is
/// turned away for the same reasons, and sees the same codes.
fn frame_refusal_message(code: SyscallErrorCode) -> String {
    let rule = match code {
        c if c == SyscallErrorCode::kSyscallErrorCodeReentrance => {
            "that address already has a frame open on this transaction's \
             stack -- nested frames need distinct addresses"
        }
        c if c == SyscallErrorCode::kSyscallErrorCodeInvokeDepthExceeded => {
            "the transaction already holds the chain's \
             max_cross_invoke_depth frames open"
        }
        c if c == SyscallErrorCode::kSyscallErrorCodeInvalidSyscallInput => {
            "the invoke flags carry bits production does not admit (only \
             TryInvoke and ReadOnly are implemented)"
        }
        _ => "production refused the invocation",
    };
    format!(
        "mock invocation frame refused: {rule} ({code}); a contract handles \
         this through Invoke (sdk::call::try_call_raw_with)"
    )
}

impl MockInvocationFrame {
    fn new(
        root_handler: *mut c_void,
        serialized_invocation: &[u8],
        invoke_flags: groundhog_xdr::invoke_flag::InvokeFlagSet,
        context_pushcount: Rc<RefCell<ThreadlocalContextPushcount>>,
        root_frame_closed: Option<Rc<Cell<bool>>>,
    ) -> Self {
        let ffi_invocation = groundhog_sys::FFISerializedInvocation {
            data: serialized_invocation.as_ptr() as *mut u8,
            len: serialized_invocation.len().try_into().unwrap(),
        };

        let res = unsafe {
            groundhog_sys::open_mock_invocation_frame(
                root_handler,
                ffi_invocation,
                invoke_flags,
            )
        };

        // Two distinct failures, and the FFI keeps them apart: an FFI code
        // means this harness called the API wrong, a syscall code means the
        // call was well-formed and production refused it. Both open no frame.
        let handler_res: groundhog_sys::FFIErrorCode =
            res.ffi_error_code.into();
        if handler_res != FFIErrorCode::None {
            // An InvokeError payload, not a plain panic!: the Unrecoverable
            // code propagates out of any enclosing frame instead of being
            // recorded as a contract panic.
            eprintln!(
                "failed to open mock invocation frame: {:?}",
                handler_res
            );
            panic_any(crate::invoke_error::unrecoverable_error());
        }
        let refused = SyscallErrorCode(res.syscall_error_code);
        if refused != SyscallErrorCode::kSyscallErrorCodeNone {
            eprintln!("{}", frame_refusal_message(refused));
            panic_any(crate::invoke_error::unrecoverable_error());
        }

        Self {
            root_handler,
            child_frame: None,
            state: MockInvocationFrameState::Started,
            context_pushcount,
            root_frame_closed,
            fault_panic: None,
        }
    }

    /// True if this frame's child frame exists and is still open at the FFI
    /// level. A frame may hold at most one open child at a time, and must
    /// not be closed before its child is.
    fn has_open_child(&self) -> bool {
        matches!(
            &self.child_frame,
            Some(child) if child.state == MockInvocationFrameState::Started
        )
    }

    /// Configure a child frame of this one, invoking `contract_address`.
    ///
    /// The address must differ from every frame already open on the stack: the
    /// test FFI admits mock frames under the same reentrance rule production
    /// applies to `Invoke`, so an address that already has an active frame is
    /// refused (hogd #3301) and `build()` panics unrecoverably. Frames that
    /// have already finished, and frames in other transactions, do not count —
    /// only the ones open at the same time.
    pub fn open_child_frame(
        &mut self,
        contract_address: groundhog_xdr::types::ContractAddress,
    ) -> MockInvocationFrameConfig<'_> {
        // One open child at a time: `FrameAction::Continue` exists to
        // interleave frames across separate transactions, not to nest
        // multiple open frames inside one parent. (The test FFI lib guards
        // this too, but replacing the open child Box would first Drop it,
        // force-finishing the wrong FFI frame before the lib could object.)
        if self.has_open_child() {
            eprintln!(
                "cannot open a child frame while another child frame is still open"
            );
            panic_any(crate::invoke_error::unrecoverable_error());
        }
        let config = MockInvocationFrameConfig::new(
            self.root_handler,
            contract_address,
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
        // Closing a frame while its child is still open is a user error.
        // It cannot be honored: `finish_mock_invocation_frame` takes no
        // frame identifier — it finishes the handler's innermost open frame
        // — so proceeding would silently finish the CHILD instead, and the
        // abandoned child's Drop would later finish what is at FFI level
        // THIS frame, poisoning the transaction.
        if self.has_open_child() {
            eprintln!(
                "cannot close a frame while its child frame is still open"
            );
            panic_any(crate::invoke_error::unrecoverable_error());
        }

        let handler_res = unsafe {
            groundhog_sys::finish_mock_invocation_frame(
                self.root_handler,
                error_code.0,
            )
        };
        let err: FFIErrorCode = handler_res.ffi_error_code.into();
        if err != FFIErrorCode::None {
            // An InvokeError payload, not a plain panic!: consumers assert
            // on the Unrecoverable payload (see
            // example-contracts/storage_errors).
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

    /// Run `lambda` as this frame's contract code, mapping panics out of it
    /// to frame outcomes. Unrecoverable [`InvokeError`] payloads (harness
    /// misuse, FFI refusals) propagate instead. The returned `FrameAction`
    /// closes the frame or leaves it open for another `execute`.
    pub fn execute(
        &mut self,
        lambda: impl FnOnce(&mut Self) -> crate::frame_builder::FrameAction
        + UnwindSafe,
    ) -> &mut Self {
        use crate::frame_builder::FrameAction;

        if self.state != MockInvocationFrameState::Started {
            eprintln!("cannot execute a frame that has already been closed");
            panic_any(crate::invoke_error::unrecoverable_error());
        }

        // Executing a frame while its child frame is still open would run
        // this frame's code over the CHILD's FFI frame (the innermost open
        // frame), misattributing syscalls, gas, and trace to the child. The
        // child must be closed first. (Holding the child's handle already
        // borrows this frame mutably, so this state is only reachable by
        // losing the handle: opening a child inside an `execute` lambda and
        // returning Continue with the child left open.)
        if self.has_open_child() {
            eprintln!(
                "cannot execute a frame while its child frame is still open"
            );
            panic_any(crate::invoke_error::unrecoverable_error());
        }

        self.push_context();

        let result = {
            // The lambda stands in for the frame's contract body -- an
            // ordinary panic out of it is classified below as this frame
            // failing deterministically -- so the hook covers it, and nests
            // with the guards `trampoline` holds around real mock bodies.
            let _hook = crate::invoke_error::PanicHookGuard::enter();
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                lambda(self)
            }))
        };

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
                    // Unrecoverable codes (harness misuse, FFI refusals)
                    // propagate back out of the close as Unrecoverable
                    // rather than becoming this frame's outcome.
                    self.close_frame_internal(my_error.error_type);
                } else {
                    // The lambda always runs with the panic hook active, so
                    // an ordinary panic payload is contract code panicking
                    // (already printed by the hook): a deterministic failure
                    // of this frame.
                    //
                    // Keep the panic before closing: this is the one point
                    // where the payload and the frame it belongs to are both
                    // in hand, on one thread, with no ambiguity about which
                    // frame faulted. The hook's record (which carries the
                    // location) is taken here too, on that same thread.
                    self.fault_panic =
                        Some(crate::invoke_error::take_panic_record(&*err));
                    self.close_frame_internal(hogexec_internal::ffi_enums::InvokeErrorCode::Deterministic);
                }
            }
        };
        self
    }

    /// The closed frame's [`FrameResult`]. Panics if the frame is still
    /// open.
    pub fn result(&mut self) -> FrameResult {
        let res = match &self.state {
            MockInvocationFrameState::Closed(output, trace, events) => {
                FrameResult::new(output.clone(), trace.clone(), events.clone())
                    .with_fault_panic(self.fault_panic.clone())
            }
            _ => {
                eprintln!(
                    "cannot get the result of a frame while it is not closed"
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

/// Configuration for a mocked transaction, applied when the handler is
/// created. Build one with [`TestEnvironment::transaction`], or use
/// [`MockTransactionConfig::default`] for the standard setup.
///
/// This is the extension point for the mocked transaction context: new knobs
/// (e.g. the source-tx hash) become fields here rather than new constructor
/// overloads.
#[derive(Clone)]
pub struct MockTransactionConfig {
    /// The auth-claim attached to the mocked transaction, observed by the
    /// contract under test (e.g. via `get_auth_claim`).
    pub auth_claim: AuthClaim,
}

impl MockTransactionConfig {
    /// A `kSourceAddressAuthData` claim with no allowed keys and zero required weight.
    pub fn default_auth_claim() -> AuthClaim {
        use groundhog_xdr::auth_claim::AuthClaim_source_address_claim;
        AuthClaim::kSourceAddressAuthData {
            source_address_claim: AuthClaim_source_address_claim {
                allowed_keys: 0,
                required_weight: 0,
            },
        }
    }
}

impl Default for MockTransactionConfig {
    fn default() -> Self {
        Self {
            auth_claim: Self::default_auth_claim(),
        }
    }
}

/// A mocked transaction context: runs one, and only one, transaction, with
/// exactly one root frame ([`call`](Self::call) /
/// [`open_root_frame`](Self::open_root_frame) panic on a second
/// invocation). Nest further calls as child frames, or use a second
/// transaction.
pub struct MockTransactionHandler {
    pub(crate) handler: *mut c_void,
    pub(crate) _env: Rc<TestEnvironment>,
    pub(crate) state: MockTransactionState,
    pub(crate) root_frame: Option<Box<MockInvocationFrame>>,
    pub(crate) context_pushcount: Rc<RefCell<ThreadlocalContextPushcount>>,
    root_frame_closed: Rc<Cell<bool>>,
}

impl MockTransactionHandler {
    /// Create a mock transaction handler with the default configuration
    /// (see [`MockTransactionConfig`]).
    ///
    /// Use [`TestEnvironment::transaction`] to customize the configuration
    /// before creating the handler.
    pub fn new(env: Rc<TestEnvironment>) -> Self {
        Self::new_with_config(env, MockTransactionConfig::default())
    }

    /// Create a mock transaction handler from an explicit
    /// [`MockTransactionConfig`], letting a test configure the context of the
    /// mocked transaction.
    pub fn new_with_config(
        env: Rc<TestEnvironment>,
        config: MockTransactionConfig,
    ) -> Self {
        let serialized_auth_claim =
            xdrs::txs::encode(&config.auth_claim).unwrap();
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

    /// True while a transaction has started but is neither committed nor
    /// aborted.
    pub(crate) fn is_running(&self) -> bool {
        self.state == MockTransactionState::Running
    }

    /// Abort the running transaction, discarding its writes.
    ///
    /// Panics unless the transaction is running and its root frame is
    /// closed.
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

    /// Attempt the two-phase commit of the running transaction.
    ///
    /// Panics unless the transaction is running and its root frame is
    /// closed.
    pub fn try_commit_transaction(&mut self) -> crate::TryCommitResult {
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

        crate::TryCommitResult::from_ffi(
            result.result_code,
            &result.serialized_conflict,
        )
    }

    /// Convenience: open a root frame targeting the given contract address.
    /// Returns a FrameBuilder for configuring and executing the frame.
    ///
    /// A transaction has exactly one root frame, so a second `call` panics;
    /// call further contracts via child frames (`FrameHandle::child_call`,
    /// [`MockInvocationFrame::open_child_frame`]) or a separate
    /// transaction.
    pub fn call(
        &mut self,
        addr: &groundhog_xdr::types::ContractAddress,
    ) -> FrameBuilder<'_> {
        let config = self.open_root_frame(*addr);
        FrameBuilder::new(config)
    }

    /// Configure this transaction's root frame, invoking `contract_address`.
    ///
    /// A transaction has exactly one root frame: calling this (or
    /// [`call`](Self::call)) a second time on the same handler panics.
    ///
    /// Child frames opened underneath it must name different addresses; see
    /// [`MockInvocationFrame::open_child_frame`].
    pub fn open_root_frame(
        &mut self,
        contract_address: groundhog_xdr::types::ContractAddress,
    ) -> MockInvocationFrameConfig<'_> {
        assert_eq!(
            self.state,
            MockTransactionState::NotStarted,
            "cannot open root frame: transaction already started"
        );
        self.state = MockTransactionState::Running;
        let config = MockInvocationFrameConfig::new(
            self.handler,
            contract_address,
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

    // Frames that are open at the same time need distinct invoked_addresses:
    // the test FFI admits mock frames under production's reentrance rule, which
    // refuses an address that already has an active frame on the stack (hogd
    // #3301). Addresses may be reused across separate handlers (separate
    // transactions, separate frame stacks) and by sequential frames at the same
    // depth, so these three cover every nesting the tests below build.
    fn root_addr() -> groundhog_xdr::types::ContractAddress {
        [0xA0u8; 32].into()
    }

    fn child_addr() -> groundhog_xdr::types::ContractAddress {
        [0xB0u8; 32].into()
    }

    fn sibling_addr() -> groundhog_xdr::types::ContractAddress {
        [0xC0u8; 32].into()
    }

    #[test]
    fn misuse_inside_nested_execute_propagates_instead_of_faulting_parent() {
        // Regression: a misuse guard firing inside an enclosing frame's
        // `execute` lambda (here: executing an already-closed child) must
        // propagate out as Unrecoverable, not be recorded as a
        // deterministic contract fault of the parent.
        let env = TestEnvironment::new(None);
        let mut handler = MockTransactionHandler::new(env);
        let frame = handler
            .open_root_frame(root_addr())
            .with_invoke_flags(0)
            .build();

        let caught =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                frame.execute(|frame| {
                    let child = frame
                        .open_child_frame(child_addr())
                        .with_invoke_flags(0)
                        .build();
                    child.execute(|_| FrameAction::Close);
                    let _ = child.result();
                    // Misuse: drive the already-closed child again.
                    child.execute(|_| FrameAction::Close);
                    FrameAction::Close
                });
            }));

        assert_unrecoverable_payload(caught.expect_err(
            "misuse inside a nested execute closure must propagate as a \
             panic, not close the parent frame as a contract fault",
        ));
    }

    #[test]
    fn test_mock_transaction_handler() {
        let env = TestEnvironment::new(None);
        let mut handler = MockTransactionHandler::new(env);

        static mut FIRST: bool = false;
        static mut SECOND: bool = false;

        let result = handler
            .open_root_frame(root_addr())
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
                handler
                    .open_root_frame(root_addr())
                    .with_invoke_flags(0)
                    .build();
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
            .open_root_frame(root_addr())
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
            .open_root_frame(root_addr())
            .with_invoke_flags(0)
            .build()
            .execute(|frame| {
                frame
                    .open_child_frame(child_addr())
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
            .open_root_frame(groundhog_xdr::types::ContractAddress::from(
                [1; 32],
            ))
            .with_invoke_flags(0)
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
            .open_root_frame(root_addr())
            .with_invoke_flags(0)
            .build()
            .execute(|frame| {
                // Outer frame: the hook covers the frame's body.
                assert!(crate::invoke_error::is_panic_hook_active());

                frame
                    .open_child_frame(child_addr())
                    .with_invoke_flags(0)
                    .build()
                    // First execute: Continue, then the child's guard drops.
                    .execute(|_| {
                        assert!(crate::invoke_error::is_panic_hook_active());
                        FrameAction::Continue
                    })
                    // Second execute: still covered inside, then Close.
                    .execute(|_| {
                        assert!(crate::invoke_error::is_panic_hook_active());
                        FrameAction::Close
                    })
                    .result();

                // The child finishing must not uncover the outer frame. This
                // is why the marker counts rather than flags.
                assert!(crate::invoke_error::is_panic_hook_active());

                FrameAction::Close
            })
            .result();

        // After everything: no frame is running.
        assert!(!crate::invoke_error::is_panic_hook_active());
    }

    #[test]
    fn test_nested_frame_panic_still_caught_by_outer() {
        let env = TestEnvironment::new(None);
        let mut handler = MockTransactionHandler::new(env);

        let result = handler
            .open_root_frame(root_addr())
            .with_invoke_flags(0)
            .build()
            .execute(|frame| {
                // Child panics
                frame
                    .open_child_frame(child_addr())
                    .with_invoke_flags(0)
                    .build()
                    .execute(|_| {
                        panic!("inner contract panic");
                    })
                    .result();

                // Outer frame should still be active and running
                assert!(crate::invoke_error::is_panic_hook_active());

                FrameAction::Close
            })
            .result();

        assert_eq!(
            result.output().status_code(),
            InvocationFrameStatusCode::Return
        );
    }

    #[test]
    fn test_interleaved_handlers_keep_panic_hook_active() {
        let env = TestEnvironment::new(None);
        let mut handler1 = MockTransactionHandler::new(env.clone());
        let mut handler2 = MockTransactionHandler::new(env);

        assert!(!crate::invoke_error::is_panic_hook_active());

        // handler1: Continue with nested child frame
        let frame1 = handler1
            .open_root_frame(root_addr())
            .with_invoke_flags(0)
            .build()
            .execute(|frame| {
                assert!(crate::invoke_error::is_panic_hook_active());

                frame
                    .open_child_frame(child_addr())
                    .with_invoke_flags(0)
                    .build()
                    .execute(|_| {
                        assert!(crate::invoke_error::is_panic_hook_active());
                        FrameAction::Close
                    })
                    .result();

                assert!(crate::invoke_error::is_panic_hook_active());
                FrameAction::Continue
            });
        assert!(!crate::invoke_error::is_panic_hook_active());

        // handler2: Continue (no children)
        let frame2 = handler2
            .open_root_frame(root_addr())
            .with_invoke_flags(0)
            .build()
            .execute(|_| {
                assert!(crate::invoke_error::is_panic_hook_active());
                FrameAction::Continue
            });
        assert!(!crate::invoke_error::is_panic_hook_active());

        // handler1: Close with nested child that panics
        let result1 = frame1
            .execute(|frame| {
                assert!(crate::invoke_error::is_panic_hook_active());

                frame
                    .open_child_frame(child_addr())
                    .with_invoke_flags(0)
                    .build()
                    .execute(|_| {
                        panic!("child panic in handler1");
                    })
                    .result();

                // The child panicking must not uncover this frame.
                assert!(crate::invoke_error::is_panic_hook_active());
                FrameAction::Close
            })
            .result();
        assert!(!crate::invoke_error::is_panic_hook_active());

        // handler2: Close with nested child doing Continue then Close
        let result2 = frame2
            .execute(|frame| {
                assert!(crate::invoke_error::is_panic_hook_active());

                frame
                    .open_child_frame(child_addr())
                    .with_invoke_flags(0)
                    .build()
                    .execute(|_| {
                        assert!(crate::invoke_error::is_panic_hook_active());
                        FrameAction::Continue
                    })
                    .execute(|_| {
                        assert!(crate::invoke_error::is_panic_hook_active());
                        FrameAction::Close
                    })
                    .result();

                assert!(crate::invoke_error::is_panic_hook_active());
                FrameAction::Close
            })
            .result();
        assert!(!crate::invoke_error::is_panic_hook_active());

        assert_eq!(
            result1.output().status_code(),
            InvocationFrameStatusCode::Return
        );
        assert_eq!(
            result2.output().status_code(),
            InvocationFrameStatusCode::Return
        );
    }

    /// Assert a caught panic payload is the unrecoverable [`InvokeError`]
    /// raised by the frame-nesting guards.
    #[track_caller]
    fn assert_unrecoverable_payload(payload: Box<dyn std::any::Any + Send>) {
        let err = payload
            .downcast_ref::<InvokeError>()
            .expect("panic payload must be an InvokeError");
        assert_eq!(
            err.error_type,
            hogexec_internal::ffi_enums::InvokeErrorCode::Unrecoverable
        );
    }

    #[test]
    fn closing_parent_with_open_child_panics_unrecoverably() {
        // Closing a frame while its child frame is still open is a clear
        // user error: `finish_mock_invocation_frame` takes no frame
        // identifier — it finishes the handler's innermost open frame — so
        // the close would silently finish the CHILD instead, and the
        // abandoned child's Drop would later finish what is at FFI level the
        // PARENT frame, poisoning the transaction. Guard it loudly.
        let env = TestEnvironment::new(None);
        let mut handler = MockTransactionHandler::new(env);
        let frame = handler
            .open_root_frame(root_addr())
            .with_invoke_flags(0)
            .build();

        let caught =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                frame.execute(|frame| {
                    // Leave the child frame open (Continue, never Close)...
                    frame
                        .open_child_frame(child_addr())
                        .with_invoke_flags(0)
                        .build()
                        .execute(|_| FrameAction::Continue);
                    // ...and close the parent anyway.
                    FrameAction::Close
                });
            }));

        assert_unrecoverable_payload(
            caught.expect_err("closing with an open child must panic"),
        );
    }

    #[test]
    fn parent_panic_with_open_child_panics_unrecoverably() {
        // Same guard via `execute()`'s Err arm: the lambda fails while a
        // child frame is still open. The frame cannot be closed out of
        // order, so the propagated panic is the unrecoverable guard error.
        let env = TestEnvironment::new(None);
        let mut handler = MockTransactionHandler::new(env);
        let frame = handler
            .open_root_frame(root_addr())
            .with_invoke_flags(0)
            .build();

        let caught =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                frame.execute(|frame| -> FrameAction {
                    frame
                        .open_child_frame(child_addr())
                        .with_invoke_flags(0)
                        .build()
                        .execute(|_| FrameAction::Continue);
                    panic!("original contract failure");
                });
            }));

        assert_unrecoverable_payload(
            caught.expect_err("failing with an open child must panic"),
        );
    }

    #[test]
    fn executing_frame_with_open_child_panics_unrecoverably() {
        // Executing a frame while its child frame is still open would run
        // the parent's code over the CHILD's FFI frame (the innermost open
        // frame), misattributing syscalls, gas, and trace to the child.
        // This state is only reachable by opening a child inside an
        // `execute` lambda and returning Continue with the child left open
        // — losing the child handle. Guard it loudly.
        let env = TestEnvironment::new(None);
        let mut handler = MockTransactionHandler::new(env);
        let frame = handler
            .open_root_frame(root_addr())
            .with_invoke_flags(0)
            .build();

        frame.execute(|frame| {
            frame
                .open_child_frame(child_addr())
                .with_invoke_flags(0)
                .build()
                .execute(|_| FrameAction::Continue);
            // Child handle is lost here, child still open.
            FrameAction::Continue
        });

        let caught =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                // Continue, not Close: the guard must fire on entry, before
                // any parent code runs over the child's FFI frame — not
                // merely when a close path happens to be reached.
                frame.execute(|_| FrameAction::Continue);
            }));

        assert_unrecoverable_payload(
            caught
                .expect_err("executing a frame with an open child must panic"),
        );
    }

    #[test]
    fn opening_second_child_while_first_open_panics_unrecoverably() {
        // A frame holds at most ONE open child frame at a time. `Continue`
        // exists to interleave frames across separate transactions, not to
        // nest multiple open frames inside one parent.
        let env = TestEnvironment::new(None);
        let mut handler = MockTransactionHandler::new(env);
        let frame = handler
            .open_root_frame(root_addr())
            .with_invoke_flags(0)
            .build();

        frame.execute(|frame| {
            // First child stays open.
            frame
                .open_child_frame(child_addr())
                .with_invoke_flags(0)
                .build()
                .execute(|_| FrameAction::Continue);
            FrameAction::Continue
        });

        let caught =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                // A fresh address, so the panic is the one-open-child guard
                // rather than anything the reentrance rule would have caught.
                frame.open_child_frame(sibling_addr());
            }));

        assert_unrecoverable_payload(caught.expect_err(
            "opening a second child while one is open must panic",
        ));
    }

    #[test]
    fn test_concurrent_transactions() {
        let env = TestEnvironment::new(None);
        let mut handler1 = MockTransactionHandler::new(env.clone());
        let mut handler2 = MockTransactionHandler::new(env);

        let frame1 = handler1
            .open_root_frame(groundhog_xdr::types::ContractAddress::from(
                [1; 32],
            ))
            .with_invoke_flags(0)
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
            .open_root_frame(groundhog_xdr::types::ContractAddress::from(
                [2; 32],
            ))
            .with_invoke_flags(0)
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

    // The point of test-ffi 0.32.0: a refused frame says which rule refused
    // it. Production hands a contract the same codes for the same reasons.
    #[test]
    fn refusal_messages_name_the_rule() {
        let msg = frame_refusal_message(
            SyscallErrorCode::kSyscallErrorCodeReentrance,
        );
        assert!(msg.contains("distinct addresses"), "{msg}");

        let msg = frame_refusal_message(
            SyscallErrorCode::kSyscallErrorCodeInvokeDepthExceeded,
        );
        assert!(msg.contains("max_cross_invoke_depth"), "{msg}");

        let msg = frame_refusal_message(
            SyscallErrorCode::kSyscallErrorCodeInvalidSyscallInput,
        );
        assert!(msg.contains("TryInvoke"), "{msg}");

        // An unnamed code still reports itself rather than going silent.
        let msg = frame_refusal_message(SyscallErrorCode(9999));
        assert!(msg.contains("9999"), "{msg}");

        // Every message points at the path that can handle the condition.
        assert!(msg.contains("try_call_raw_with"), "{msg}");
    }

    // The diagnostic only reaches the developer if the panic carrying it
    // gets out. An enclosing frame's catch site records any other payload
    // as a deterministic fault in its own body, which would close the
    // parent as failed and leave the refusal unreported.
    #[test]
    fn a_refused_frame_propagates_instead_of_faulting_its_parent() {
        let env = TestEnvironment::new(None);
        let mut handler = MockTransactionHandler::new(env);
        let frame = handler
            .open_root_frame(root_addr())
            .with_invoke_flags(0)
            .build();

        let caught =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                frame.execute(|frame| {
                    // Reentrance: the root's address already has a frame
                    // open on this stack.
                    frame
                        .open_child_frame(root_addr())
                        .with_invoke_flags(0)
                        .build();
                    FrameAction::Close
                });
            }));

        assert_unrecoverable_payload(caught.expect_err(
            "a refused frame must propagate as unrecoverable, not close \
             the parent frame as a contract fault",
        ));
    }

    // Invoke flags reach the frame with their bits intact: a child opened
    // ReadOnly refuses the write that the same child, opened without flags,
    // makes.
    #[test]
    fn read_only_invoke_flag_refuses_writes_in_the_frame() {
        let env = TestEnvironment::new(None);
        env.setup_grant_storage_tokens(&child_addr(), 1_000_000_000);
        let mut handler = MockTransactionHandler::new(env);
        let key =
            groundhog_xdr::types::StorageKeyContractData::new([0x5A; 32]);
        let read_only = groundhog_xdr::invoke_flag::InvokeFlag::ReadOnly
            .0
            .cast_unsigned();

        let result = handler
            .open_root_frame(root_addr())
            .with_invoke_flags(0)
            .build()
            .execute(|frame| {
                for (flags, admitted) in [(read_only, false), (0, true)] {
                    let child = frame
                        .open_child_frame(child_addr())
                        .with_invoke_flags(flags)
                        .build();
                    child.execute(|_| {
                        let res = groundhog_xdr::syscalls::raw_BytestringSet(
                            &key, b"x",
                        );
                        if admitted {
                            res.unwrap();
                        } else {
                            assert!(res.unwrap_err().is_invalid_input());
                        }
                        FrameAction::Close
                    });
                    // A failed assertion above faults the child rather than
                    // failing the test, so check how it closed.
                    assert_eq!(
                        child.result().output().status_code(),
                        InvocationFrameStatusCode::Return,
                        "flags {flags:#x}",
                    );
                }
                FrameAction::Close
            })
            .result();
        assert_eq!(
            result.output().status_code(),
            InvocationFrameStatusCode::Return
        );
    }
}
