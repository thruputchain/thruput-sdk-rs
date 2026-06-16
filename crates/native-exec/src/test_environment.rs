use crate::frame_result::FrameResult;
use crate::mock_contract::{MockContract, MockContractInner};
use crate::mock_transaction_handler::MockTransactionHandler;
use crate::transact_result::TransactResult;
use core::ffi::c_void;
use groundhog_storage_cost::StorageCostModel;
use groundhog_sys::FFIErrorCode;
use groundhog_xdr::address_auth::AddressAuthData;
use groundhog_xdr::contract::ContractEntrypointID;
use groundhog_xdr::contract::*;
use groundhog_xdr::storage_item::WireStorageItem;
use groundhog_xdr::transaction::*;
use groundhog_xdr::types::*;
use hogexec_internal::ffi_enums::InvokeErrorCode;
use sha2::{Digest, Sha256};
use std::cell::RefCell;
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

/// An in-process groundhog instance for testing contracts natively.
///
/// A `TestEnvironment` owns a fresh in-memory database and runtime context
/// (backed by hogd via FFI). Constructors return a [`TestEnvironmentHandle`];
/// clone the handle to share the environment across helpers.
///
/// - [`TestEnvironment::new`] takes an optional storage cost model;
///   [`TestEnvironment::no_cost`] disables storage costs entirely.
/// - The `setup_*` methods seed state (contracts, deployments, storage
///   tokens) as if applied in a previous block; call them before running
///   transactions.
/// - [`TestEnvironment::transact`] executes a transaction against the
///   environment and auto-commits; build frames with the fluent
///   `tx.call(&addr)` API. [`TestEnvironment::advance_block`] makes
///   committed effects visible to subsequent transactions.
///
/// ```no_run
/// use groundhog_native_exec::test_environment::TestEnvironment;
/// use groundhog_xdr::types::ContractAddress;
///
/// let env = TestEnvironment::no_cost();
/// let addr = ContractAddress::new([0x11; 32]);
/// let result = env.transact(|tx| tx.call(&addr).run(|| { /* guest code */ }));
/// result.assert_success();
/// ```
///
/// Setting the `SDK_TEST_ENABLE_SYSCALL_LOGGING` environment variable turns
/// on syscall tracing for environments created afterwards.
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

    pub fn get_snapshot_storage_item(
        &self,
        addr: &ContractAddress,
        key: &StorageKeyContractData,
    ) -> Option<WireStorageItem> {
        let addrkey = AddressAndKey {
            address: *addr,
            key: StorageKey::ContractData {
                contract_data_key: *key,
            },
        };
        let serialized_addrkey = txs::encode(&addrkey).unwrap();
        let ffi_addrkey = groundhog_sys::FFISerializedAddressAndKey {
            data: serialized_addrkey.as_ptr(),
            len: serialized_addrkey.len().try_into().unwrap(),
        };

        let res = unsafe {
            groundhog_sys::get_snapshot_storage_item(self.db, ffi_addrkey)
        };
        let err: FFIErrorCode = res.ffi_error_code.into();
        if err != FFIErrorCode::None {
            panic!("failed to get snapshot storage item: {err}");
        }

        res.data
            .as_slice()
            .map(|bytes| txs::decode::<WireStorageItem>(bytes).unwrap())
    }
}

impl Drop for DBWrapper {
    fn drop(&mut self) {
        let r = unsafe { groundhog_sys::free_db(self.db) };
        let err: FFIErrorCode = r.into();
        if err != FFIErrorCode::None {
            crate::drop_failure(format_args!("failed to free db: {:?}", err));
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
            crate::drop_failure(format_args!(
                "failed to close runtime context: {:?}",
                err
            ));
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
        if res != 0 {
            let err: groundhog_sys::FFIErrorCode = res.into();
            if err == groundhog_sys::FFIErrorCode::ContractVerificationFailed {
                panic!(
                    "setup_create_contract failed: {err}\n\
                     \n\
                     The contract's WASM binary failed validation. Common causes:\n\
                     - Missing stack limit instrumentation (WASM not run through groundhog-wasm-meter)\n\
                     - Linear memory bounds too large\n\
                     - Unsupported WASM extension (e.g. threads, SIMD, reference types)\n"
                );
            }
            panic!("setup_create_contract failed: {err}");
        }
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
                wasmi_contract.data.len() as u32
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

/// Shared handle to a [`TestEnvironment`]; returned by its constructors.
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

    pub fn get_snapshot_storage_item(
        self: &TestEnvironmentHandle,
        addr: &ContractAddress,
        key: &StorageKeyContractData,
    ) -> Option<WireStorageItem> {
        self.inner.borrow().db.get_snapshot_storage_item(addr, key)
    }

    /// Internal: create an empty mock contract. Use `register_mock` (or the
    /// generated `register_mock_contract`) instead.
    #[doc(hidden)]
    pub fn add_contract(self: &TestEnvironmentHandle) -> MockContract {
        let inner = self.inner.borrow_mut().add_contract();
        MockContract::new(inner, self.clone())
    }

    /// Create a mock contract, register methods onto it, and make it
    /// deployable in the test environment.
    ///
    /// The registrar may be a single generated `__mock` function or a
    /// closure that combines multiple interface registrars onto one mock.
    pub fn register_mock<F>(self: &TestEnvironmentHandle, f: F) -> MockContract
    where
        F: FnOnce(&mut MockContract),
    {
        let mut c = self.add_contract();
        f(&mut c);
        self.setup_create_contract(&c.as_contract());
        c
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

    // -- Setup-phase methods --
    //
    // The setup_* methods seed groundhog state directly, bypassing
    // transaction execution. Call them before running any transactions in
    // a test; their effects appear as if applied in a previous block.

    /// Setup phase: create a contract in groundhog state (not deployed at
    /// any address). Returns the contract hash. Run before any
    /// transactions in the test.
    pub fn setup_create_contract(
        self: &TestEnvironmentHandle,
        contract: &Contract,
    ) -> SHA256Hash {
        self.inner.borrow_mut().setup_create_contract(contract)
    }

    /// Setup phase: deploy an already-created contract at `addr`. Run
    /// before any transactions in the test.
    pub fn setup_create_deployment(
        self: &TestEnvironmentHandle,
        addr: &ContractAddress,
        contract: &Contract,
    ) {
        self.inner
            .borrow_mut()
            .setup_create_deployment(addr, contract)
    }

    /// Setup phase: grant storage tokens to the contract at `addr`, as if
    /// granted in a previous block. Like `setup_create_contract` and
    /// `setup_create_deployment`, run before any transactions in the test.
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

        let res = unsafe {
            groundhog_sys::verify_signed_transaction_auth(
                ffi_serialized_tx,
                ffi_serialized_source_auth_data,
            )
        };
        let err: FFIErrorCode = res.ffi_error_code.into();
        if err != FFIErrorCode::None {
            panic!("failed to run auth checks: {:?}", err);
        }
        res.auth_error_code.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use groundhog_xdr::storage_item::StorageDeltaFlags;

    const TEST_ADDR: ContractAddress = ContractAddress::new([0x11; 32]);

    #[test]
    fn get_snapshot_storage_item_reads_committed_bytestring() {
        let env = TestEnvironment::new(None);
        let key = StorageKeyContractData::new([0x22; 32]);
        env.setup_grant_storage_tokens(&TEST_ADDR, 1_000_000_000);

        let result = env.transact(|tx| {
            tx.call(&TEST_ADDR).run(|| {
                groundhog_xdr::syscalls::raw_BytestringSet(
                    &key,
                    b"hello",
                    StorageDeltaFlags::None.0,
                )
                .unwrap();
            })
        });
        result.assert_success();

        env.advance_block();

        let item = env.get_snapshot_storage_item(&TEST_ADDR, &key);
        assert_eq!(
            item,
            Some(WireStorageItem::kItemTypeBytestring {
                bytestring: b"hello".to_vec().try_into().unwrap(),
            }),
        );
    }

    #[test]
    fn get_snapshot_storage_item_returns_none_for_missing_key() {
        let env = TestEnvironment::new(None);
        let key = StorageKeyContractData::new([0x33; 32]);

        assert_eq!(env.get_snapshot_storage_item(&TEST_ADDR, &key), None);
    }

    #[test]
    fn get_snapshot_storage_item_reads_committed_bytestring_on_deployed_address()
     {
        let env = TestEnvironment::new(None);
        let key = StorageKeyContractData::new([0x44; 32]);
        env.setup_grant_storage_tokens(&TEST_ADDR, 1_000_000_000);
        env.inner.borrow_mut().setup_create_deployment(
            &TEST_ADDR,
            &groundhog_xdr::contract::Contract::kNative {
                native_contract_id:
                    groundhog_xdr::precompiles::HogexecPrecompiledNativeContracts::kNativeContractHogToken,
            },
        );

        let result = env.transact(|tx| {
            tx.call(&TEST_ADDR).run(|| {
                groundhog_xdr::syscalls::raw_BytestringSet(
                    &key,
                    b"hello",
                    StorageDeltaFlags::None.0,
                )
                .unwrap();
            })
        });
        result.assert_success();

        env.advance_block();

        let item = env.get_snapshot_storage_item(&TEST_ADDR, &key);
        assert_eq!(
            item,
            Some(WireStorageItem::kItemTypeBytestring {
                bytestring: b"hello".to_vec().try_into().unwrap(),
            }),
        );
    }
}
