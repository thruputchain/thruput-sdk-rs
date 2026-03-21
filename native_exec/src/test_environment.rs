use crate::frame_result::FrameResult;
use crate::mock_contract::{MockContract, MockContractInner};
use crate::mock_transaction_handler::MockTransactionHandler;
use crate::transact_result::TransactResult;
use crate::transaction_handler::TransactionHandler;
use core::ffi::c_void;
use groundhog_storage_cost::StorageCostModel;
use groundhog_sys::FFIErrorCode;
use groundhog_xdr::address_auth::AddressAuthData;
use groundhog_xdr::contract::ContractEntrypointID;
use groundhog_xdr::contract::*;
use groundhog_xdr::transaction::*;
use groundhog_xdr::types::*;
use hogexec_internal::ffi_enums::InvokeErrorCode;
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::panic::UnwindSafe;
use std::rc::Rc;
use xdrs::txs;

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
    pub(crate) db: DBWrapper,
    pub(crate) contract_context: RuntimeContextWrapper,
    mock_contract_index: i32,
    contracts: Vec<*mut MockContractInner>, // owns these contracts
}

pub struct TestEnvironment {
    pub(crate) inner: RefCell<TestEnvironmentInner>,
}

unsafe extern "C" fn trampoline(
    context: *mut c_void,
    method: u64,
) -> groundhog_sys::ForeignInvokeResult {
    let c: *mut MockContractInner = unsafe { core::mem::transmute(context) };

    let method: ContractEntrypointID = method.to_be_bytes().into();

    match unsafe { (*c).get_func(method) } {
        Some(x) => x(),
        None => groundhog_sys::ForeignInvokeResult {
            panic: InvokeErrorCode::Deterministic.0,
        },
    }
}

pub(crate) struct DBWrapper {
    pub(crate) db: *mut c_void,
}

impl DBWrapper {
    pub fn new(cost_model: Option<&StorageCostModel>) -> Self {
        let cost_model_bytes =
            cost_model.map(|model| model.serialize().unwrap());

        let ffi_cost_model_bytes: (*const u8, u32) = cost_model_bytes
            .as_ref()
            .map(|vec| (vec.as_ptr(), vec.len().try_into().unwrap()))
            .unwrap_or((core::ptr::null(), 0));

        let res = unsafe {
            groundhog_sys::create_db(
                ffi_cost_model_bytes.0,
                ffi_cost_model_bytes.1,
            )
        };
        let err: FFIErrorCode = res.ffi_error_code.into();
        if err != FFIErrorCode::None {
            panic!("failed to create db: {}", res.ffi_error_code);
        }
        assert!(!res.db.is_null());
        Self { db: res.db }
    }

    pub fn get_cost_model(&self) -> StorageCostModel {
        let res = unsafe { groundhog_sys::get_cost_model(self.db) };
        let err: FFIErrorCode = res.ffi_error_code.into();
        if err != FFIErrorCode::None {
            panic!("failed to get cost model: {}", res.ffi_error_code);
        }
        if res.data.len == 0 {
            panic!("failed to get cost model: no data");
        }

        let data = xdrs::txs::decode::<
            groundhog_xdr::storage_cost::StorageCostModelXdr,
        >(res.data.as_slice().unwrap())
        .unwrap();
        StorageCostModel::new(data)
    }
}

impl Drop for DBWrapper {
    fn drop(&mut self) {
        let r = unsafe { groundhog_sys::free_db(self.db) };
        let err: FFIErrorCode = r.into();
        if err != FFIErrorCode::None {
            panic!("failed to free db: {:?}", err);
        }
    }
}

pub(crate) struct RuntimeContextWrapper {
    pub context: *mut c_void,
}

impl RuntimeContextWrapper {
    pub fn new() -> Self {
        let res = unsafe { groundhog_sys::open_runtime_context() };
        let err: FFIErrorCode = res.ffi_error_code.into();
        if err != FFIErrorCode::None {
            panic!("failed to create runtime context: {}", res.ffi_error_code);
        }
        assert!(!res.context.is_null());
        Self {
            context: res.context,
        }
    }
}

impl Drop for RuntimeContextWrapper {
    fn drop(&mut self) {
        let r = unsafe { groundhog_sys::close_runtime_context(self.context) };
        let err: FFIErrorCode = r.into();
        if err != FFIErrorCode::None {
            panic!("failed to close runtime context: {:?}", err);
        }
    }
}

impl TestEnvironmentInner {
    pub fn new(cost_model: Option<&StorageCostModel>) -> Self {
        // the initialization in create_db() advances the block once, so it is prepped for
        // accepting new transactions (in proposal mode)
        Self {
            db: DBWrapper::new(cost_model),
            contract_context: RuntimeContextWrapper::new(),
            mock_contract_index: 0,
            contracts: vec![],
        }
    }

    pub fn enable_syscall_logging(level: groundhog_sys::FFILogLevel) {
        unsafe { groundhog_sys::enable_syscall_logging(level as i32) };
    }

    /// Creates a new contract within groundhog.
    /// This new contract is not deployed at any particular address.
    ///
    /// In the test framework, the semantics of the setup_{create,deploy}_contract functions
    /// are that after the function is called, the effects on the database are as though that
    /// contract had been {created,deployed} during the previous block.
    ///
    /// This contract can be subsequently deployed, either by the setup_create_deployment function,
    /// or by the syscalls within a transaction.
    ///
    /// Returns the hash of the contract, which is used to identify the contract when deploying it.
    ///
    /// Note: Even if a mock contract is added to the test environment (via add_contract),
    /// this function must be called to create the contract within groundhog.
    pub fn setup_create_contract(
        &mut self,
        contract: &Contract,
    ) -> SHA256Hash {
        let contract_bytes = txs::encode(&contract).unwrap();
        let res = unsafe {
            groundhog_sys::setup_create_contract(
                self.contract_context.context,
                self.db.db,
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

    /// Deploys the specified contract at the specified address.
    /// The contract must already have been created, either by a transaction in an earlier block
    /// or (in the test framework) by the setup_create_contract function.
    ///
    /// In the test framework, the semantics of the setup_{create,deploy}_contract functions
    /// are that after the function is called, the effects on the database are as though that
    /// contract had been {created,deployed} during the previous block.
    ///
    /// This address can be subsequently invoked.
    ///
    /// This contract will fail if the contract hash does not exist as an existing contract within groundhog
    /// (i.e. if the contract was not created by the setup_create_contract function,
    /// or was not created by a transaction in an earlier block).
    pub fn setup_create_deployment(
        &mut self,
        addr: &ContractAddress,
        contract: &Contract,
    ) {
        let contract_bytes = txs::encode(&contract).unwrap();

        let mut hasher = Sha256::new();
        hasher.update(&contract_bytes);
        let res: [u8; 32] = hasher.finalize().into();
        let contract_hash: SHA256Hash = res.into();

        let contract_size = match contract {
            Contract::kNative { .. } => 0,
            Contract::kMockTest { .. } => TEST_CONTRACT_SIZE,
            Contract::kWasmi { wasmi_contract } => {
                wasmi_contract.wasm.len() as u32
            }
            other => panic!("unsupported contract variant: {:?}", other),
        };

        let res = unsafe {
            groundhog_sys::setup_deploy_contract(
                self.db.db,
                addr.as_ptr(),
                contract_hash.as_ptr(),
                contract_size,
            )
        };
        let err: FFIErrorCode = res.into();
        if err != FFIErrorCode::None {
            panic!("failed to setup new contract deployment: {:?}", err);
        }
    }

    /// Grants the specified number of storage tokens to the specified contract address.
    ///
    /// In the test framework, the semantics of this function is that
    /// after this function is called, the effects on the database are as though that
    /// contract had been granted the specified number of storage tokens during the previous block.
    ///
    /// This function should not be called once the contract has been deployed and invoked;
    /// the internal accounting of storage tokens will be incorrect if that occurs.
    pub fn setup_grant_storage_tokens(
        &mut self,
        addr: &ContractAddress,
        tokens: i64,
    ) {
        let res = unsafe {
            groundhog_sys::setup_grant_contract_storage_tokens(
                self.db.db,
                addr.as_ptr(),
                tokens,
            )
        };
        let err: FFIErrorCode = res.into();
        if err != FFIErrorCode::None {
            panic!("failed to setup grant storage tokens: {:?}", err);
        }
    }

    /// Advances the test state to the next block.
    /// This makes the effects of the previous block visible to the next block.
    /// Only the transactions that were successfully committed will be visible to the next block.
    /// An error may occur if there are any transactions that executed but did not commit or abort.
    pub fn advance_block(&mut self) {
        let res = unsafe { groundhog_sys::advance_new_block(self.db.db) };
        let err: FFIErrorCode = res.into();
        if err != FFIErrorCode::None {
            panic!("failed to advance block: {:?}", err);
        }
    }

    /// Registers a new mock contract with the test environment.
    /// This does not create the contract as something visible within Groundhog,
    /// nor does it deploy it at a particular address.
    ///
    /// This contract (a Contract(ContractType::kMockTest))
    /// can subsequently be created by the setup_create_contract functions,
    /// or by being attached as one of the newly created contracts
    /// on a SignedTransaction.
    pub fn add_contract(&mut self) -> *mut MockContractInner {
        let out: *mut MockContractInner = Box::into_raw(Box::new(
            MockContractInner::new(self.mock_contract_index),
        ));

        let new_id = self.mock_contract_index;

        self.contracts.push(out);
        self.mock_contract_index += 1;

        let res = unsafe {
            groundhog_sys::register_new_contract(
                self.contract_context.context,
                new_id,
                Some(trampoline),
                out as *mut c_void,
            )
        };
        let err: FFIErrorCode = res.into();
        if err != FFIErrorCode::None {
            panic!("failed to link fn: {:?}", err);
        }

        out
    }

    pub fn get_current_cost_model(&self) -> StorageCostModel {
        self.db.get_cost_model()
    }

    pub fn run_auth_checks(
        &self,
        tx: &SignedTransaction,
    ) -> groundhog_xdr::auth_claim::AuthVerifyErrorCode {
        let serialized_tx = txs::encode(tx).unwrap();
        let ffi_serialized_tx =
            groundhog_sys::FFISerializedSignedTransaction {
                data: serialized_tx.as_ptr() as *mut u8,
                len: serialized_tx.len().try_into().unwrap(),
            };
        let res = unsafe {
            groundhog_sys::verify_signed_transaction_auth_from_db(
                self.db.db,
                ffi_serialized_tx,
            )
        };
        let err: FFIErrorCode = res.ffi_error_code.into();
        if err != FFIErrorCode::None {
            panic!("failed to run auth checks: {:?}", err);
        }
        res.auth_error_code.into()
    }
}

impl Drop for TestEnvironmentInner {
    fn drop(&mut self) {
        for contract in &self.contracts {
            let _ = unsafe { Box::from_raw(*contract) };
        }
    }
}

impl Default for TestEnvironmentInner {
    fn default() -> Self {
        Self::new(None)
    }
}

pub type TestEnvironmentHandle = Rc<TestEnvironment>;

impl TestEnvironment {
    pub fn new(
        cost_model: Option<&StorageCostModel>,
    ) -> TestEnvironmentHandle {
        let te = Rc::new(Self {
            inner: RefCell::new(TestEnvironmentInner::new(cost_model)),
        });

        if std::env::var("SDK_TEST_ENABLE_SYSCALL_LOGGING").is_ok() {
            Self::enable_syscall_logging();
        }

        te
    }

    #[allow(clippy::should_implement_trait)]
    pub fn default() -> TestEnvironmentHandle {
        Self::new(None)
    }

    pub fn no_cost() -> TestEnvironmentHandle {
        Self::new(Some(
            &groundhog_storage_cost::StorageCostModel::no_cost_model(),
        ))
    }

    // This has effect only after creation of a test environment
    pub fn enable_syscall_logging() {
        TestEnvironmentInner::enable_syscall_logging(
            groundhog_sys::FFILogLevel::Trace,
        );
    }

    pub fn advance_block(self: &TestEnvironmentHandle) {
        self.inner.borrow_mut().advance_block()
    }

    pub fn get_current_cost_model(
        self: &TestEnvironmentHandle,
    ) -> StorageCostModel {
        self.inner.borrow().get_current_cost_model()
    }

    pub fn add_contract(self: &TestEnvironmentHandle) -> MockContract {
        let inner = self.inner.borrow_mut().add_contract();
        MockContract::new(inner, self.clone())
    }

    pub fn prepare_test<F>(
        self: &TestEnvironmentHandle,
        test: F,
    ) -> TransactionHandler
    where
        F: Fn() + UnwindSafe + Clone + 'static,
    {
        let mut test_contract = self.add_contract();
        let entrypoint = ContractEntrypointID::from([0; 8]);
        test_contract.register_func(entrypoint, test);
        self.inner
            .borrow_mut()
            .setup_create_contract(&test_contract.as_contract());

        let invoke_addr: ContractAddress = [0xFF; 32].into();

        self.inner.borrow_mut().setup_create_deployment(
            &invoke_addr,
            &test_contract.as_contract(),
        );

        let stx = SignedTransaction {
            signatures: vec![].try_into().unwrap(),
            tx: Transaction {
                uniquifier: Uniquifier {
                    block_expiry_height: 0,
                    nonce: 0,
                },
                auth_claims: vec![].try_into().unwrap(),
                gas_claims: vec![].try_into().unwrap(),
                invocation: Invocation {
                    invoked_address: invoke_addr,
                    entrypoint,
                    gas_limit: u64::MAX,
                    calldata: [].try_into().unwrap(),
                },
                contracts_to_deploy: vec![].try_into().unwrap(),
            },
        };

        TransactionHandler::new(self.clone(), &stx)
    }

    /// Execute a transaction with auto-commit.
    ///
    /// Creates a MockTransactionHandler, gives the closure a mutable reference,
    /// and always calls try_commit_transaction when the closure returns
    /// (hogd handles abort internally when the root frame reverted).
    pub fn transact(
        self: &TestEnvironmentHandle,
        f: impl FnOnce(&mut MockTransactionHandler) -> FrameResult,
    ) -> TransactResult {
        let mut tx = MockTransactionHandler::new(self.clone());
        let frame_result = f(&mut tx);
        let exec_result = tx.try_commit_transaction();
        TransactResult::new(frame_result, exec_result)
    }

    /// Execute a transaction with manual commit/abort control.
    ///
    /// The closure receives a MockTransactionHandler and is responsible for
    /// calling tx.try_commit_transaction() or tx.abort_transaction().
    /// The closure's return value is forwarded as the return value of transact_manual.
    pub fn transact_manual<R>(
        self: &TestEnvironmentHandle,
        f: impl FnOnce(&mut MockTransactionHandler) -> R,
    ) -> R {
        let mut tx = MockTransactionHandler::new(self.clone());
        f(&mut tx)
    }

    pub fn setup_create_contract(
        self: &TestEnvironmentHandle,
        contract: &Contract,
    ) -> SHA256Hash {
        self.inner.borrow_mut().setup_create_contract(contract)
    }

    pub fn setup_create_deployment(
        self: &TestEnvironmentHandle,
        addr: &ContractAddress,
        contract: &Contract,
    ) {
        self.inner
            .borrow_mut()
            .setup_create_deployment(addr, contract)
    }

    pub fn setup_grant_storage_tokens(
        self: &TestEnvironmentHandle,
        addr: &ContractAddress,
        tokens: i64,
    ) {
        self.inner
            .borrow_mut()
            .setup_grant_storage_tokens(addr, tokens)
    }

    /// Check the auth claims on the provided transaction
    /// against the auth data loaded from the database.
    ///
    /// May perform additional checks (such as checking block height limits)
    /// not performed by the static version.  Will additionally fail if
    /// addresses without auth data are referenced.
    pub fn verify_signed_transaction_auth(
        self: &TestEnvironmentHandle,
        tx: &SignedTransaction,
    ) -> groundhog_xdr::auth_claim::AuthVerifyErrorCode {
        self.inner.borrow().run_auth_checks(tx)
    }

    /// Check the auth claims on the provided transaction
    /// against the provided source and auxiliary auth data.
    /// Does not load any information from the database.
    pub fn verify_signed_transaction_auth_static(
        tx: &SignedTransaction,
        source_auth_data: &AddressAuthData,
        aux_auth_data: &[(ContractAddress, AddressAuthData)],
    ) -> groundhog_xdr::auth_claim::AuthVerifyErrorCode {
        let serialized_tx = txs::encode(tx).unwrap();
        let ffi_serialized_tx =
            groundhog_sys::FFISerializedSignedTransaction {
                data: serialized_tx.as_ptr() as *mut u8,
                len: serialized_tx.len().try_into().unwrap(),
            };

        let serialized_source_auth_data =
            txs::encode(source_auth_data).unwrap();
        let ffi_serialized_source_auth_data =
            groundhog_sys::FFISourceAuthData {
                data: serialized_source_auth_data.as_ptr() as *mut u8,
                len: serialized_source_auth_data.len().try_into().unwrap(),
            };

        let serialized_aux_auth_data: Vec<(ContractAddress, Vec<u8>)> =
            aux_auth_data
                .iter()
                .map(|x| (x.0, txs::encode(&x.1).unwrap()))
                .collect();

        let ffi_serialized_aux_auth_data: Vec<
            groundhog_sys::FFISerializedAuthData,
        > = serialized_aux_auth_data
            .iter()
            .map(|x| groundhog_sys::FFISerializedAuthData {
                address: *x.0,
                data: x.1.as_ptr() as *mut u8,
                len: x.1.len().try_into().unwrap(),
            })
            .collect();

        let ffi_auxiliary_auth_data = groundhog_sys::FFIAuxiliaryAuthData {
            auth_data: ffi_serialized_aux_auth_data.as_ptr()
                as *mut groundhog_sys::FFISerializedAuthData,
            count: ffi_serialized_aux_auth_data.len().try_into().unwrap(),
        };

        let res = unsafe {
            groundhog_sys::verify_signed_transaction_auth(
                ffi_serialized_tx,
                ffi_serialized_source_auth_data,
                ffi_auxiliary_auth_data,
            )
        };
        let err: FFIErrorCode = res.ffi_error_code.into();
        if err != FFIErrorCode::None {
            panic!("failed to run auth checks: {:?}", err);
        }
        res.auth_error_code.into()
    }
}
