use crate::bindings::*;
use crate::test_environment::TestEnvironment;
use core::ffi::c_void;
use groundhog_xdr::transaction::SignedTransaction;
use std::marker::PhantomData;
use xdrs::serialize_txs;

// Can run one, and only one, transaction,
// and then get the results
pub struct TransactionHandler<'a> {
    handler: *mut c_void,
    _marker: PhantomData<&'a TestEnvironment>,
    // cannot outlive the environment.

    // TODO(geoff): we could swap the backing pointer to the db
    // to be a shared ptr, and have handler hold a reference,
    // like we do with the mock contract context.
    invoked: bool,
}

impl<'a> TransactionHandler<'a> {
    pub fn new(env: &'a TestEnvironment, stx: &SignedTransaction) -> Self {
        let tx_bytes = serialize_txs::encode(stx).unwrap();
        let out = unsafe {
            Self {
                handler: open_syscall_handler(
                    env.db,
                    env.contract_context,
                    tx_bytes.as_ptr(),
                    tx_bytes.len().try_into().unwrap(),
                ),
                _marker: PhantomData,
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
}

impl Drop for TransactionHandler<'_> {
    fn drop(&mut self) {
        unsafe { close_syscall_handler(self.handler) }
    }
}
