use crate::rv32_stack_usage::Rv32StackUsage;
use crate::test_environment::TestEnvironment;
use core::ffi::c_void;
use std::rc::Rc;
use thruput_trace::InvocationFrameTrace;
use thruput_xdr::event::TxEmittedEvent;
use thruput_xdr::transaction::InvocationFrameOutput;
use thruput_xdr::transaction::SignedTransaction;
use xdrs::txs;

use thruput_sys::FFIErrorCode;

/// Executes one, and only one, full [`SignedTransaction`] against the
/// environment: [`root_invoke`](Self::root_invoke), then
/// [`try_commit_transaction`](Self::try_commit_transaction) or
/// [`abort_transaction`](Self::abort_transaction).
pub struct TransactionHandler {
    handler: *mut c_void,
    _env: Rc<TestEnvironment>,
    stx: SignedTransaction,
    invoked: bool,
    committed: bool,
    aborted: bool,
}

/// Result of a transaction's root invocation ([`TransactionHandler::root_invoke`]).
pub struct RootInvokeResult {
    /// The root frame's outcome.
    pub output: InvocationFrameOutput,
    /// Gas consumed by the invocation.
    pub gas_consumed: u64,
    /// The execution trace, when tracing was enabled.
    pub trace: Option<InvocationFrameTrace>,
    /// Events emitted by the transaction.
    pub events: Vec<TxEmittedEvent>,
    /// The stack records of the rv32 invocations the transaction ran, in
    /// the order they finished: a callee's before its caller's. When the
    /// root invocation targets an rv32 contract its own record is the last;
    /// when it targets a mock contract, every record is a call made under
    /// it. Empty if it ran none. See [`Rv32StackUsage`].
    pub rv32_stack_usage_records: Vec<Rv32StackUsage>,
}

impl RootInvokeResult {
    /// The most recent of
    /// [`rv32_stack_usage_records`](Self::rv32_stack_usage_records): the
    /// root invocation's own stack record when it targets an rv32 contract,
    /// else the last rv32 call made under it. `None` if the transaction ran
    /// none.
    pub fn rv32_stack_usage(&self) -> Option<Rv32StackUsage> {
        self.rv32_stack_usage_records.last().copied()
    }
}

impl TransactionHandler {
    /// Open a handler for executing `stx` against `env`.
    pub fn new(env: Rc<TestEnvironment>, stx: &SignedTransaction) -> Self {
        let tx_bytes = txs::encode(stx).unwrap();

        let handler_res = unsafe {
            thruput_sys::open_syscall_handler(
                env.inner.borrow().db.db,
                env.inner.borrow().contract_context.context,
                thruput_sys::FFISerializedSignedTransaction {
                    data: tx_bytes.as_ptr() as *mut u8,
                    len: tx_bytes.len().try_into().unwrap(),
                },
            )
        };
        let err: FFIErrorCode = handler_res.ffi_error_code.into();
        if err != FFIErrorCode::None {
            panic!("failed to open syscall handler: {:?}", err);
        }
        assert!(!handler_res.syscall_handler.is_null());
        Self {
            handler: handler_res.syscall_handler,
            _env: env.clone(),
            stx: stx.clone(),
            invoked: false,
            committed: false,
            aborted: false,
        }
    }

    /// Invokes the transaction represented by the TransactionHandler.
    /// This must be called before other methods that rely on actions taken within the transaction.
    /// This method must only be called once per transaction.
    ///
    /// Returns the result of the root invocation.
    /// This does NOT commit the transaction to the database.
    /// The caller should subsequently call try_commit_transaction or abort_transaction
    /// before advancing to the next block.
    ///
    /// try_commit_transaction checks the result of the root invocation,
    /// and will abort the transaction if the root invocation failed.
    /// So it is safe to call either try_commit_transaction or abort_transaction
    /// after root_invoke, regardless of whether the root invocation succeeded or failed.
    pub fn root_invoke(&mut self) -> RootInvokeResult {
        if self.invoked {
            panic!("cannot double invoke on a TransactionHandler")
        }
        self.invoked = true;
        assert!(!self.committed && !self.aborted);

        // No panic-hook activation here: contract bodies activate it on entry
        // (see `PanicHookGuard`), and nothing else under this call is contract
        // code -- rv32 contracts run in the interpreter, and the rest is this
        // crate's own marshalling, where a panic is a harness bug that should
        // stay unrecoverable and print in full.
        let foreign_result: thruput_sys::ForeignTracedInvokeResult = unsafe {
            thruput_sys::root_invoke(self.handler, /*tracing*/ 1)
        };

        let err: FFIErrorCode = foreign_result.ffi_error_code.into();
        if err != FFIErrorCode::None {
            panic!("FFI error: failed to invoke transaction: {err}");
        }

        let trace = foreign_result.serialized_trace.as_slice().map(|s| {
            txs::decode(s)
                .unwrap_or_else(|e| panic!("failed to decode trace: {}", e))
        });

        let output = foreign_result.serialized_output.as_slice().map(|s| {
            txs::decode(s)
                .unwrap_or_else(|e| panic!("failed to decode output: {}", e))
        });

        let events: Vec<TxEmittedEvent> = if let Some(events_data) =
            foreign_result.serialized_events.as_slice()
        {
            let decoded: xdr_common::VecM<TxEmittedEvent> =
                txs::decode(events_data).unwrap_or_else(|e| {
                    panic!(
                        "failed to decode events ({} bytes): {}",
                        events_data.len(),
                        e
                    )
                });
            decoded.into()
        } else {
            Vec::new()
        };

        RootInvokeResult {
            output: output.unwrap(),
            gas_consumed: foreign_result.gas_consumed,
            trace,
            events,
            // Every record on the handler is this transaction's: the
            // handler is opened for the one transaction, and the root
            // invocation is the only thing that runs on it.
            rv32_stack_usage_records: crate::rv32_stack_usage::records(
                self.handler,
            ),
        }
    }

    /// The signed transaction this handler executes.
    pub fn get_signed_transaction(&self) -> &SignedTransaction {
        &self.stx
    }

    /// Tries to commit the transaction to the database.
    /// This must be called after root_invoke, and only once per transaction.
    ///
    /// Returns the [`TryCommitResult`](crate::TryCommitResult) reported by the
    /// executor; check it to see whether the commit succeeded. On a
    /// nondeterministic (two-phase commit) failure it carries the
    /// storage-conflict diagnostic.
    ///
    /// This will abort the transaction if the root invocation failed.
    /// So it is safe to call either try_commit_transaction or abort_transaction
    /// after root_invoke, regardless of whether the root invocation succeeded or failed.
    ///
    /// # Panics
    ///
    /// Panics if called before `root_invoke`, or if the transaction was
    /// already committed or aborted.
    pub fn try_commit_transaction(&mut self) -> crate::TryCommitResult {
        if !self.invoked {
            panic!(
                "cannot try to commit a transaction that has not been invoked"
            );
        }
        if self.committed || self.aborted {
            panic!(
                "cannot try to commit a transaction that has already been committed or aborted"
            );
        }
        self.committed = true;
        let res = unsafe { thruput_sys::try_commit_transaction(self.handler) };
        let err: FFIErrorCode = res.ffi_error_code.into();
        if err != FFIErrorCode::None {
            panic!("failed to try to commit transaction: {err}");
        }

        crate::TryCommitResult::from_ffi(
            res.result_code,
            &res.serialized_conflict,
        )
    }

    /// Aborts the transaction.
    /// This can only be called after root_invoke, and can only be called once per transaction.
    pub fn abort_transaction(&mut self) {
        if !self.invoked {
            panic!("cannot abort a transaction that has not been invoked");
        }
        if self.committed || self.aborted {
            panic!(
                "cannot abort a transaction that has already been committed or aborted"
            );
        }
        self.aborted = true;

        let res = unsafe { thruput_sys::abort_transaction(self.handler) };
        let err: FFIErrorCode = res.into();
        if err != FFIErrorCode::None {
            panic!("failed to abort transaction: {:?}", err);
        }
    }
}

impl Drop for TransactionHandler {
    fn drop(&mut self) {
        if self.invoked && !self.committed && !self.aborted {
            // Abort the uncommitted transaction. Inlined rather than calling
            // `abort_transaction` to avoid its panic on FFI error: we may be
            // unwinding from a test failure, and a panic in `drop` would
            // double-panic into a SIGABRT that hides the original message.
            self.aborted = true;
            let res = unsafe { thruput_sys::abort_transaction(self.handler) };
            let err: FFIErrorCode = res.into();
            if err != FFIErrorCode::None {
                crate::drop_failure(format_args!(
                    "failed to abort uncommitted transaction during cleanup: {:?}",
                    err
                ));
            }
        }

        let res = unsafe { thruput_sys::close_syscall_handler(self.handler) };
        let err: FFIErrorCode = res.into();
        if err != FFIErrorCode::None {
            crate::drop_failure(format_args!(
                "failed to close syscall handler: {:?}",
                err
            ));
        }
    }
}
