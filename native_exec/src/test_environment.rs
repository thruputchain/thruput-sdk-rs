use crate::bindings::*;
use crate::invoke_error::set_override_panic_hook;
use crate::mock_contract::{MockContract, MockContractInner};
use crate::transaction_handler::TransactionHandler;
use core::ffi::c_void;
use std::cell::RefCell;
use std::rc::Rc;
use groundhog_xdr::contract::*;
use groundhog_xdr::ffi_enums::InvokeErrorCode;
use groundhog_xdr::contract::ContractEntrypointID;
use groundhog_xdr::transaction::*;
use groundhog_xdr::types::*;
use sha2::{Digest, Sha256};
use std::panic::UnwindSafe;
use xdrs::serialize_txs;

// TODO(geoff): I cannot figure out how to prevent rust from moving objects
// without just converting back to raw pointers.
// Pin<Box<T>> with PhantomPinned still wound up getting moved, somehow.
// It would be nice to avoid this later of indirection, as that would make passing
// (or at least seem to make) passing closures in as parameters to test functions
// much easier.

// When deploying a new contract, hogd accounts for the storage cost of the contract
// based on its size.  We don't have access to contract size here (because it's not built),
// so we pick a default (nonzero) value as a placeholder.
// Nonzero so that contract tests will expect to at least pay some storage cost when deploying a new contract.
const TEST_CONTRACT_SIZE: u32 = 1024;
pub struct TestEnvironmentInner {
    pub(crate) db: *mut c_void,
    pub(crate) contract_context: *mut c_void,
    mock_contract_index: i32,
    contracts: Vec<*mut MockContractInner>, // owns these contracts
}

pub struct TestEnvironment {
    pub(crate) inner: RefCell<TestEnvironmentInner>,
}

unsafe extern "C" fn trampoline(
    context: *mut c_void,
    method: u64,
) -> ForeignInvokeResult {
    let c: *mut MockContractInner = unsafe { core::mem::transmute(context) };

    let method: ContractEntrypointID = method.to_be_bytes().into();

    match unsafe { (*c).get_func(method) } {
        Some(x) => {
            return x();
        }
        None => {
            return ForeignInvokeResult {
                panic: InvokeErrorCode::kInvokeDeterministic as i32,
            }
        }
    }
}

impl TestEnvironmentInner {
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

        // the initialization in create_db() advances the block once, so it is prepped for
        // accepting new transactions (in proposal mode)

        out
    }

    pub fn enable_syscall_logging() {
        unsafe { enable_syscall_logging() };
    }

    pub fn setup_create_contract(
        &mut self,
        contract: &Contract,
    ) -> SHA256Hash {
        let contract_bytes = serialize_txs::encode(&contract).unwrap();
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
        res.into()
    }

    pub fn setup_create_deployment(
        &mut self,
        addr: &ContractAddress,
        contract: &Contract,
    ) {
        let contract_bytes = serialize_txs::encode(&contract).unwrap();

        let mut hasher = Sha256::new();
        hasher.update(&contract_bytes);
        let res: [u8; 32] = hasher.finalize().into();
        let contract_hash: SHA256Hash = res.into();

        let contract_size = match contract {
            Contract::kNative { .. } => 0,
            Contract::kMockTest { .. } => TEST_CONTRACT_SIZE,
            Contract::kWasmi { wasmi_contract } => wasmi_contract.wasm.len() as u32,
            Contract::Default { contract_type } => panic!("unsupported contract type {}", contract_type),
        };

        let res = unsafe {
            setup_deploy_contract(
                self.db,
                addr.as_ptr(),
                contract_hash.as_ptr(),
                contract_size,
            )
        };
        assert_eq!(res, 0);
    }

    pub fn setup_grant_storage_tokens(
        &mut self,
        addr: &ContractAddress,
        tokens: i64,
    ) {
        unsafe {
            setup_grant_contract_storage_tokens(self.db, addr.as_ptr(), tokens)
        }
    }

    pub fn advance_block(&mut self) {
        unsafe { advance_new_block(self.db) }
    }

    pub fn add_contract(&mut self) -> *mut MockContractInner {
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

        out
    }
}

impl Drop for TestEnvironmentInner {
    fn drop(&mut self) {
        unsafe { free_db(self.db) };
        unsafe { close_runtime_context(self.contract_context) };
        for contract in &self.contracts {
            let _ = unsafe { Box::from_raw(*contract) };
        }
    }
}

impl TestEnvironment {
    pub fn new() -> Rc<Self> {
        Rc::new(Self {
            inner: RefCell::new(TestEnvironmentInner::new()),
        })
    }

    pub fn enable_syscall_logging() {
        TestEnvironmentInner::enable_syscall_logging();
    }

    pub fn add_contract(self: &Rc<TestEnvironment>) -> MockContract {
        let inner = self.inner.borrow_mut().add_contract();
        MockContract::new(inner, self.clone())
    }

    pub fn prepare_test<F>(
        self: &Rc<TestEnvironment>,
        test: F,
    ) -> TransactionHandler
    where
        F: Fn() -> () + UnwindSafe + Clone + 'static,
    {
        let mut test_contract = self.add_contract();
        let entrypoint = ContractEntrypointID::from([0; 8]);
        test_contract.register_func(entrypoint, test);
        self.inner.borrow_mut().setup_create_contract(&test_contract.as_contract());

        let invoke_addr: ContractAddress = [0xFF; 32].into();

        self.inner.borrow_mut().setup_create_deployment(
            &invoke_addr,
            &test_contract.as_contract(),
        );

        let stx = SignedTransaction {
            signatures: vec![].try_into().unwrap(),
            signature_weight: 0,
            aux_signatures: vec![].try_into().unwrap(),
            signature_weight: 0,
            tx: Transaction {
                uniquifier: Uniquifier {
                    block_expiry_height: 0,
                    nonce: 0,
                },
                invocation: Invocation {
                    invoked_address: invoke_addr,
                    entrypoint: entrypoint,
                    gas_limit: u64::MAX,
                    gas_claims: [].try_into().unwrap(),
                    calldata: [].try_into().unwrap(),
                },
                contracts_to_deploy: vec![].try_into().unwrap(),
            },
        };

        TransactionHandler::new(self.clone(), &stx)
    }

    pub fn setup_create_contract(self: &Rc<TestEnvironment>, contract: &Contract) -> SHA256Hash {
        self.inner.borrow_mut().setup_create_contract(contract)
    }

    pub fn setup_create_deployment(self: &Rc<TestEnvironment>, addr: &ContractAddress, contract: &Contract) {
        self.inner.borrow_mut().setup_create_deployment(addr, contract)
    }

    pub fn setup_grant_storage_tokens(self: &Rc<TestEnvironment>, addr: &ContractAddress, tokens: i64) {
        self.inner.borrow_mut().setup_grant_storage_tokens(addr, tokens)
    }
    
    
}
