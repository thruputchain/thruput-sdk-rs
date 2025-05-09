use crate::bindings::*;
use crate::invoke_error::set_override_panic_hook;
use crate::mock_contract::{MockContract, MockContractInner};
use crate::transaction_handler::TransactionHandler;
use core::ffi::c_void;
use groundhog_xdr::contract::*;
use groundhog_xdr::ffi_enums::InvokeErrorCode;
use groundhog_xdr::serialize::{Limits, WriteXdr};
use groundhog_xdr::transaction::*;
use groundhog_xdr::types::*;
use sha2::{Digest, Sha256};
use std::panic::UnwindSafe;

// TODO(geoff): I cannot figure out how to prevent rust from moving objects
// without just converting back to raw pointers.
// Pin<Box<T>> with PhantomPinned still wound up getting moved, somehow.
// It would be nice to avoid this later of indirection, as that would make passing
// (or at least seem to make) passing closures in as parameters to test functions
// much easier.
pub struct TestEnvironment {
    pub(crate) db: *mut c_void,
    pub(crate) contract_context: *mut c_void,
    mock_contract_index: u32,
    contracts: Vec<*mut MockContractInner>, // owns these contracts
}

unsafe extern "C" fn trampoline(
    context: *mut c_void,
    method: u32,
) -> ForeignInvokeResult {
    let c: *mut MockContractInner = unsafe { core::mem::transmute(context) };

    match unsafe { (*c).get_func(method) } {
        Some(x) => {
            return x();
        }
        None => {
            return ForeignInvokeResult {
                result_code: 0,
                panic: InvokeErrorCode::kInvokeDeterministic as i32,
            }
        }
    }
}

impl<'a> TestEnvironment {
    pub fn new() -> Self {
        set_override_panic_hook();
        let out = Self {
            db: unsafe { create_db() },
            contract_context: unsafe { open_runtime_context() },
            mock_contract_index: 0,
            contracts: vec![],
        };

        if out.db == core::ptr::null_mut() {
            panic!("failed to initialize database!");
        }
        if out.contract_context == core::ptr::null_mut() {
            panic!("failed to initialize contract context!");
        }

        // prepare to accept transactions on the first block
        unsafe { advance_new_block(out.db) };

        out
    }

    pub fn enable_syscall_logging() {
        unsafe { enable_syscall_logging() };
    }

    pub fn setup_create_contract(
        &mut self,
        contract: &Contract,
    ) -> SHA256Hash {
        let contract_bytes = contract.to_xdr(Limits::none()).unwrap();
        let res = unsafe {
            setup_create_contract(
                self.contract_context,
                self.db,
                contract_bytes.as_ptr(),
                contract_bytes.len().try_into().unwrap(),
            )
        };
        assert_eq!(res, 0);
        let mut hasher = Sha256::new();
        hasher.update(&contract_bytes);
        let res: [u8; 32] = hasher.finalize().into();
        let bytearray: ByteArray<32> = res.into();
        bytearray.into()
    }

    pub fn setup_create_deployment(
        &mut self,
        addr: &ContractAddress,
        contract: &Contract,
    ) {
        let contract_bytes = contract.to_xdr(Limits::none()).unwrap();

        let mut hasher = Sha256::new();
        hasher.update(&contract_bytes);
        let res: [u8; 32] = hasher.finalize().into();
        let bytearray: ByteArray<32> = res.into();
        let contract_hash: SHA256Hash = bytearray.into();

        let res = unsafe {
            setup_deploy_contract(
                self.db,
                addr.as_ptr(),
                contract_hash.as_ptr(),
            )
        };
        assert_eq!(res, 0);
    }

    pub fn advance_block(&mut self) {
        unsafe { advance_new_block(self.db) }
    }

    pub fn add_contract(&mut self) -> MockContract<'a> {
        let out: *mut MockContractInner = Box::into_raw(Box::new(
            MockContractInner::new(self.mock_contract_index),
        ));

        let new_id = self.mock_contract_index;

        self.contracts.push(out);
        self.mock_contract_index += 1;

        let res = unsafe {
            register_new_contract(
                self.contract_context,
                new_id,
                Some(trampoline),
                out as *mut c_void,
            )
        };
        if res != 0 {
            panic!("failed to link fn");
        }
        MockContract::new(out)
    }

    pub fn prepare_test<F>(
        self: &'a mut TestEnvironment,
        test: F,
    ) -> TransactionHandler<'a>
    where
        F: Fn() -> i32 + 'a + UnwindSafe + Clone + 'static,
    {
        let mut test_contract = self.add_contract();
        test_contract.register_func(0, test);
        self.setup_create_contract(&test_contract.as_contract());

        let invoke_addr: ContractAddress = [0xFF; 32].into();

        self.setup_create_deployment(
            &invoke_addr,
            &test_contract.as_contract(),
        );

        let stx = SignedTransaction {
            signature: Signature::default(),
            tx: Transaction {
                created: 0,
                id: [0; 16].into(),
                invocation: Invocation {
                    invoked_address: invoke_addr,
                    method: 0,
                    gas_limit: u64::MAX,
                    calldata: [].try_into().unwrap(),
                },
                contracts_to_deploy: vec![].try_into().unwrap(),
            },
        };

        TransactionHandler::new(self, &stx)
    }
}

impl Drop for TestEnvironment {
    fn drop(&mut self) {
        unsafe { free_db(self.db) };
        unsafe { close_runtime_context(self.contract_context) };
        for contract in &self.contracts {
            let _ = unsafe { Box::from_raw(*contract) };
        }
    }
}
