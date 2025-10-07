use crate::bindings::*;
use crate::test_environment::TestEnvironment;
use core::ffi::c_void;
use std::rc::Rc;
use groundhog_xdr::transaction::SignedTransaction;
use xdrs::serialize_txs;
use groundhog_xdr::event::TxEventLog;

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
        let out = unsafe {
            Self {
                handler: open_syscall_handler(
                    env.inner.borrow().db,
                    env.inner.borrow().contract_context,
                    tx_bytes.as_ptr(),
                    tx_bytes.len().try_into().unwrap(),
                ),
                _env: env.clone(),
                stx: stx.clone(),
                invoked: false,
            }
        };
        if out.handler == core::ptr::null_mut() {
            panic!("failed to initialize handler!");
        }
        out
    }

    pub fn root_invoke(&mut self) -> ForeignRootInvokeResult {
        if self.invoked {
            panic!("cannot double invoke on a TransactionHandler")
        }
        self.invoked = true;

        unsafe { root_invoke(self.handler) }
    }

    pub fn get_event_logs(&self) -> Option<TxEventLog> {
        let buffer_len : u32 = unsafe { get_event_log_len(self.handler) };
        if buffer_len == 0 {
            return None;
        }

        let mut buffer = vec![0u8; buffer_len as usize];

        unsafe { get_event_log(self.handler, buffer.as_mut_ptr(), buffer_len) };

        let decoded : TxEventLog = serialize_txs::decode(&buffer).unwrap();

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
        unsafe { close_syscall_handler(self.handler) }
    }
}
