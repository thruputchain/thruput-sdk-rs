use crate::bindings::*;
use crate::test_environment::TestEnvironment;
use core::ffi::c_void;
use groundhog_xdr::event::TxEventLog;
use groundhog_xdr::transaction::SignedTransaction;
use std::rc::Rc;
use xdrs::serialize_txs;

use crate::test_environment::FFIErrorCode;

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
}

impl TransactionHandler {
    pub fn new(env: Rc<TestEnvironment>, stx: &SignedTransaction) -> Self {
        let tx_bytes = serialize_txs::encode(stx).unwrap();

        let handler_res = unsafe {
            open_syscall_handler(
                env.inner.borrow().db.db,
                env.inner.borrow().contract_context.context,
                tx_bytes.as_ptr(),
                tx_bytes.len().try_into().unwrap(),
            )
        };
        let err: FFIErrorCode = handler_res.ffi_error_code.into();
        if err != FFIErrorCode::None {
            panic!("failed to open syscall handler: {:?}", err);
        }
        assert!(handler_res.syscall_handler != core::ptr::null_mut());
        Self {
            handler: handler_res.syscall_handler,
            _env: env.clone(),
            stx: stx.clone(),
            invoked: false,
        }
    }

    pub fn root_invoke(&mut self) -> ForeignRootInvokeResult {
        if self.invoked {
            panic!("cannot double invoke on a TransactionHandler")
        }
        self.invoked = true;

        unsafe { root_invoke(self.handler) }
    }

    pub fn get_event_logs(&self) -> Option<TxEventLog> {
        let buffer_len_res = unsafe { get_event_log_len(self.handler) };
        let err: FFIErrorCode = buffer_len_res.ffi_error_code.into();
        if err != FFIErrorCode::None {
            panic!("failed to get event log length: {:?}", err);
        }
        let buffer_len: u32 = buffer_len_res.len;
        if buffer_len == 0 {
            return None;
        }

        let mut buffer = vec![0u8; buffer_len as usize];

        let event_log_res = unsafe {
            get_event_log(self.handler, buffer.as_mut_ptr(), buffer_len)
        };
        let err: FFIErrorCode = event_log_res.into();
        if err != FFIErrorCode::None {
            panic!("failed to get event log: {:?}", err);
        }

        let decoded: TxEventLog = serialize_txs::decode(&buffer).unwrap();

        Some(decoded)
    }

    pub fn get_signed_transaction(&self) -> &SignedTransaction {
        &self.stx
    }

    pub fn try_commit_transaction(&self) -> bool {
        let res = unsafe { try_commit_transaction(self.handler) };
        if res.panic != 0 {
            panic!("try_commit_transaction failed: {}", res.panic);
        }
        res.result_code == 1
    }
}

impl Drop for TransactionHandler {
    fn drop(&mut self) {
        let res = unsafe { close_syscall_handler(self.handler) };
        let err: FFIErrorCode = res.into();
        if err != FFIErrorCode::None {
            panic!("failed to close syscall handler: {:?}", err);
        }
    }
}
