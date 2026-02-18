use crate::test_environment::TestEnvironment;
use core::ffi::c_void;
use groundhog_trace::InvocationFrameTrace;
use groundhog_xdr::event::TxEventLog;
use groundhog_xdr::transaction::InvocationFrameOutput;
use groundhog_xdr::transaction::SignedTransaction;
use std::rc::Rc;
use xdrs::txs;

use groundhog_sys::FFIErrorCode;

// Can run one, and only one, transaction,
// and then get the results
pub struct TransactionHandler {
    handler: *mut c_void,
    _env: Rc<TestEnvironment>,
    stx: SignedTransaction,
    // cannot outlive the environment.

    // TODO(geoff): we could swap the backing pointer to the db
    // to be a shared ptr, and have handler hold a reference,
    // like we do with the mock contract context.
    invoked: bool,
    committed: bool,
    aborted: bool,
}

pub struct RootInvokeResult {
    pub output: InvocationFrameOutput,
    pub gas_consumed: u64,
    pub trace: Option<InvocationFrameTrace>,
}

impl TransactionHandler {
    pub fn new(env: Rc<TestEnvironment>, stx: &SignedTransaction) -> Self {
        let tx_bytes = txs::encode(stx).unwrap();

        let handler_res = unsafe {
            groundhog_sys::open_syscall_handler(
                env.inner.borrow().db.db,
                env.inner.borrow().contract_context.context,
                groundhog_sys::FFISerializedSignedTransaction {
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

        let foreign_result: groundhog_sys::ForeignRootInvokeResult = unsafe {
            groundhog_sys::root_invoke(self.handler, /*tracing*/ 1)
        };

        if foreign_result.ffi_error_code != FFIErrorCode::None as u32 {
            panic!(
                "FFI error: failed to invoke transaction: {:?}",
                foreign_result.ffi_error_code
            );
        }

        let trace = foreign_result.serialized_trace.as_slice().map(|s| {
            txs::decode(s)
                .unwrap_or_else(|e| panic!("failed to decode trace: {}", e))
        });

        let output = foreign_result.serialized_output.as_slice().map(|s| {
            txs::decode(s)
                .unwrap_or_else(|e| panic!("failed to decode output: {}", e))
        });

        RootInvokeResult {
            output: output.unwrap(),
            gas_consumed: foreign_result.gas_consumed,
            trace,
        }
    }

    pub fn get_event_logs(&self) -> Option<TxEventLog> {
        if !self.invoked {
            panic!(
                "cannot get event logs for a transaction that has not been invoked"
            );
        }
        if !self.committed && !self.aborted {
            panic!(
                "cannot get event logs for a transaction that has not been committed or aborted"
            );
        }

        let buffer_len_res =
            unsafe { groundhog_sys::get_event_log_len(self.handler) };
        let err: FFIErrorCode = buffer_len_res.ffi_error_code.into();
        if err == FFIErrorCode::EventLogNotFound {
            return None;
        }
        if err != FFIErrorCode::None {
            panic!("failed to get event log length: {:?}", err);
        }
        let buffer_len: u32 = buffer_len_res.len;
        if buffer_len == 0 {
            return None;
        }

        let mut buffer = vec![0u8; buffer_len as usize];

        let event_log_res = unsafe {
            groundhog_sys::get_event_log(
                self.handler,
                buffer.as_mut_ptr(),
                buffer_len,
            )
        };
        let err: FFIErrorCode = event_log_res.into();
        if err != FFIErrorCode::None {
            panic!("failed to get event log: {:?}", err);
        }

        let decoded: TxEventLog = txs::decode(&buffer).unwrap_or_else(|e| {
            panic!(
                "failed to decode event log ({} bytes): {}",
                buffer.len(),
                e
            )
        });

        Some(decoded)
    }

    pub fn get_signed_transaction(&self) -> &SignedTransaction {
        &self.stx
    }

    /// Tries to commit the transaction to the database.
    /// This must be called after root_invoke, and only once per transaction.
    ///
    /// Returns true if the transaction commits, false otherwise.
    ///
    /// This will abort the transaction if the root invocation failed.
    /// So it is safe to call either try_commit_transaction or abort_transaction
    /// after root_invoke, regardless of whether the root invocation succeeded or failed.
    pub fn try_commit_transaction(
        &mut self,
    ) -> groundhog_xdr::exec_result::ExecResult {
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
        let res =
            unsafe { groundhog_sys::try_commit_transaction(self.handler) };
        if res.ffi_error_code != FFIErrorCode::None as u32 {
            panic!(
                "failed to try to commit transaction: {:?}",
                res.ffi_error_code
            );
        }

        let exec_result: groundhog_xdr::exec_result::ExecResult =
            res.result_code.try_into().unwrap();
        exec_result
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

        let res = unsafe { groundhog_sys::abort_transaction(self.handler) };
        let err: FFIErrorCode = res.into();
        if err != FFIErrorCode::None {
            panic!("failed to abort transaction: {:?}", err);
        }
    }
}

impl Drop for TransactionHandler {
    fn drop(&mut self) {
        if self.invoked && !self.committed && !self.aborted {
            self.abort_transaction();
        }

        let res =
            unsafe { groundhog_sys::close_syscall_handler(self.handler) };
        let err: FFIErrorCode = res.into();
        if err != FFIErrorCode::None {
            panic!("failed to close syscall handler: {:?}", err);
        }
    }
}
