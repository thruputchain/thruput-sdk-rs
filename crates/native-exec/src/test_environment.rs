use crate::frame_result::FrameResult;
use crate::mock_contract::{MockContract, MockContractInner};
use crate::mock_transaction_handler::MockTransactionHandler;
use crate::test_environment_builder::TestEnvironmentBuilder;
use crate::transact_result::TransactResult;
use crate::transaction_builder::TransactionBuilder;
use core::ffi::c_void;
use hogexec_internal::ffi_enums::InvokeErrorCode;
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::rc::Rc;
use thruput_storage_cost::StorageCostModel;
use thruput_sys::FFIErrorCode;
use thruput_xdr::address_auth::AddressAuthData;
use thruput_xdr::contract::ContractEntrypointID;
use thruput_xdr::contract::*;
use thruput_xdr::network_config::NetworkConfig;
use thruput_xdr::storage_item::WireStorageItem;
use thruput_xdr::transaction::*;
use thruput_xdr::types::*;
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

/// Name of the network that [`TestEnvironment`]s run on.
///
/// Every environment is created against one fixed, obviously-not-mainnet
/// network identity, so that a test cannot accidentally produce (or replay)
/// something signed for a real network.
pub const TEST_NETWORK_NAME: &str = "LOCAL_UNIT_TEST";

/// The [`NetworkHash`] that [`TestEnvironment`]s are created with:
/// `sha256(`[`TEST_NETWORK_NAME`]`)`.
///
/// Pass a different one to [`TestEnvironment::with_network_hash`] if a test
/// needs to pin a specific network.
pub fn test_network_hash() -> NetworkHash {
    let mut hasher = Sha256::new();
    hasher.update(TEST_NETWORK_NAME.as_bytes());
    let hash: [u8; 32] = hasher.finalize().into();
    NetworkHash::new(hash.into())
}

/// The state behind a [`TestEnvironment`]: the FFI database, runtime
/// context, and registered mock contracts.
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
///   [`TestEnvironment::no_cost`] disables storage costs entirely. Both run on
///   the default test network ([`test_network_hash`]);
///   [`TestEnvironment::with_network_hash`] picks a different one.
/// - The `setup_*` methods seed state (contracts, deployments, storage
///   tokens) as if applied in a previous block; call them before running
///   transactions.
/// - [`TestEnvironment::transact`] executes a transaction against the
///   environment and auto-commits; build frames with the fluent
///   `tx.call(&addr)` API. [`TestEnvironment::advance_block`] makes
///   committed effects visible to subsequent transactions.
/// - [`TestEnvironment::builder`] reaches settings the constructors above
///   do not, notably the chain's network config parameters. It is entirely
///   optional: everything else here behaves the same whether or not any
///   test uses it.
///
/// ```no_run
/// use thruput_native_exec::test_environment::TestEnvironment;
/// use thruput_xdr::types::ContractAddress;
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
) -> thruput_sys::ForeignInvokeResult {
    let c: *mut MockContractInner = unsafe { core::mem::transmute(context) };

    let method: ContractEntrypointID = method.to_be_bytes().into();

    // Undefined method: run the registered fallback if there is one (see
    // `MockContract::register_unknown_method`), otherwise fail the frame
    // deterministically — the native analog of the rv32 dispatcher's
    // `ebreak`.
    match unsafe { (*c).get_func(method).or_else(|| (*c).get_unknown_func()) }
    {
        Some(x) => {
            // Mark this thread as running a contract body for the call. The
            // body may resume on a pool thread rather than the one that
            // activated the panic hook, and this is the point where the right
            // thread is known without guessing: it is the one calling `x`.
            let _in_body = crate::invoke_error::PanicHookGuard::enter();
            x()
        }
        None => thruput_sys::ForeignInvokeResult {
            panic: InvokeErrorCode::Deterministic.0,
        },
    }
}

pub(crate) struct DBWrapper {
    pub(crate) db: *mut c_void,
}

impl DBWrapper {
    pub fn new(
        cost_model: Option<&StorageCostModel>,
        network_hash: &NetworkHash,
        genesis_root_contract: &Contract,
        network_config: Option<&NetworkConfig>,
    ) -> Self {
        let cost_model_bytes =
            cost_model.map(|model| model.serialize().unwrap());

        let ffi_cost_model_bytes: (*const u8, u32) = cost_model_bytes
            .as_ref()
            .map(|vec| (vec.as_ptr(), vec.len().try_into().unwrap()))
            .unwrap_or((core::ptr::null(), 0));

        let genesis_root_bytes = txs::encode(genesis_root_contract).unwrap();

        let network_config_bytes =
            network_config.map(|config| txs::encode(config).unwrap());
        let ffi_network_config_bytes: (*const u8, u32) = network_config_bytes
            .as_ref()
            .map(|vec| (vec.as_ptr(), vec.len().try_into().unwrap()))
            .unwrap_or((core::ptr::null(), 0));

        // hogd reads exactly 32 bytes from this pointer (it is not XDR), and
        // rejects a null one. A null network config takes this build's stock
        // genesis values; a non-null one must state a gapless prefix of the
        // parameter id space, which hogd checks fatally (SIGABRT, no return
        // through the FFI) -- build it with
        // `crate::network_config::NetworkConfigBuilder`, which refuses to
        // produce anything else.
        let res = unsafe {
            thruput_sys::create_db(
                ffi_cost_model_bytes.0,
                ffi_cost_model_bytes.1,
                network_hash.as_ptr(),
                genesis_root_bytes.as_ptr(),
                genesis_root_bytes.len().try_into().unwrap(),
                ffi_network_config_bytes.0,
                ffi_network_config_bytes.1,
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
        let res = unsafe { thruput_sys::get_cost_model(self.db) };
        let err: FFIErrorCode = res.ffi_error_code.into();
        if err != FFIErrorCode::None {
            panic!("failed to get cost model: {}", res.ffi_error_code);
        }
        if res.data.len == 0 {
            panic!("failed to get cost model: no data");
        }

        let data = xdrs::txs::decode::<
            thruput_xdr::storage_cost::StorageCostModelXdr,
        >(res.data.as_slice().unwrap())
        .unwrap();
        StorageCostModel::new(data)
    }

    pub fn get_network_config(&self) -> NetworkConfig {
        let res = unsafe { thruput_sys::get_network_config(self.db) };
        let err: FFIErrorCode = res.ffi_error_code.into();
        if err != FFIErrorCode::None {
            panic!("failed to get network config: {}", res.ffi_error_code);
        }
        if res.data.len == 0 {
            panic!("failed to get network config: no data");
        }

        xdrs::txs::decode::<NetworkConfig>(res.data.as_slice().unwrap())
            .unwrap()
    }

    pub fn get_snapshot_storage_item(
        &self,
        addr: &ContractAddress,
        key: &StorageKeyContractData,
    ) -> Option<WireStorageItem> {
        self.get_snapshot_item(
            addr,
            StorageKey::ContractData {
                contract_data_key: *key,
            },
        )
    }

    pub fn get_snapshot_auth_data(
        &self,
        addr: &ContractAddress,
    ) -> Option<AddressAuthData> {
        match self.get_snapshot_item(addr, StorageKey::AuthData)? {
            WireStorageItem::kItemTypeAuthData { auth_data } => {
                Some(auth_data)
            }
            other => panic!("auth data key holds a non-auth item: {other:?}"),
        }
    }

    fn get_snapshot_item(
        &self,
        addr: &ContractAddress,
        key: StorageKey,
    ) -> Option<WireStorageItem> {
        let addrkey = AddressAndKey {
            address: *addr,
            key,
        };
        let serialized_addrkey = txs::encode(&addrkey).unwrap();
        let ffi_addrkey = thruput_sys::FFISerializedAddressAndKey {
            data: serialized_addrkey.as_ptr(),
            len: serialized_addrkey.len().try_into().unwrap(),
        };

        let res = unsafe {
            thruput_sys::get_snapshot_storage_item(self.db, ffi_addrkey)
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
        let r = unsafe { thruput_sys::free_db(self.db) };
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
        let res = unsafe { thruput_sys::open_runtime_context() };
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
        let r = unsafe { thruput_sys::close_runtime_context(self.context) };
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
    /// Create a fresh in-memory database (with the given cost model) and
    /// runtime context on `network_hash`. `genesis_root_contract` is
    /// installed as the genesis root deployment, and `network_config`
    /// states the execution parameters -- `None` for the stock ones.
    pub fn new(
        cost_model: Option<&StorageCostModel>,
        network_hash: &NetworkHash,
        genesis_root_contract: &Contract,
        network_config: Option<&NetworkConfig>,
    ) -> Self {
        // the initialization in create_db() advances the block once, so it is prepped for
        // accepting new transactions (in proposal mode)
        Self {
            db: DBWrapper::new(
                cost_model,
                network_hash,
                genesis_root_contract,
                network_config,
            ),
            contract_context: RuntimeContextWrapper::new(),
            mock_contract_index: 0,
            contracts: vec![],
        }
    }

    /// Turn on FFI-level syscall logging to stderr at `level` for
    /// environments created afterwards.
    pub fn enable_syscall_logging(level: thruput_sys::FFILogLevel) {
        unsafe { thruput_sys::enable_syscall_logging(level as i32) };
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
            thruput_sys::setup_create_contract(
                self.contract_context.context,
                self.db.db,
                contract_bytes.as_ptr(),
                contract_bytes.len().try_into().unwrap(),
            )
        };
        if res != 0 {
            let err: thruput_sys::FFIErrorCode = res.into();
            if err == thruput_sys::FFIErrorCode::ContractVerificationFailed {
                panic!(
                    "setup_create_contract failed: {err}\n\
                     \n\
                     The contract's rv32 artifact failed validation. Common causes:\n\
                     - The raw ELF was deployed instead of the lowered .rv32a image\n\
                     - Memory image bounds too large\n\
                     - Unsupported instructions (only the RV32IM subset is accepted)\n"
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

        let contract_size = deployment_contract_size(contract);

        let res = unsafe {
            thruput_sys::setup_deploy_contract(
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
            thruput_sys::setup_grant_contract_storage_tokens(
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
    ///
    /// The new block takes its timepoint from the wall clock. Use
    /// [`TestEnvironmentInner::advance_block_at`] to pin one instead.
    pub fn advance_block(&mut self) {
        self.advance_block_inner(std::ptr::null())
    }

    /// Advances to the next block as [`TestEnvironmentInner::advance_block`]
    /// does, but pins the new block's timepoint rather than reading the clock.
    ///
    /// Blocks only move forward in time: `block_timepoint` must be at or after
    /// the open block's, and the host refuses an earlier one.
    pub fn advance_block_at(&mut self, block_timepoint: TimePoint) {
        let raw: u64 = block_timepoint.into_inner();
        self.advance_block_inner(&raw)
    }

    fn advance_block_inner(&mut self, block_timepoint: *const u64) {
        let res = unsafe {
            thruput_sys::advance_new_block(self.db.db, block_timepoint)
        };
        let err: FFIErrorCode = res.into();
        if err == FFIErrorCode::InvalidArgument {
            panic!(
                "failed to advance block: the requested timepoint precedes the \
                 block it would follow. Blocks only move forward in time; pin a \
                 timepoint at or after the current block's."
            );
        }
        if err != FFIErrorCode::None {
            panic!("failed to advance block: {:?}", err);
        }
    }

    /// Registers a new mock contract with the test environment.
    /// This does not create the contract as something visible within Thruput,
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
            thruput_sys::register_new_contract(
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

    /// The storage cost model the environment's database is running with.
    pub fn get_current_cost_model(&self) -> StorageCostModel {
        self.db.get_cost_model()
    }

    /// The execution parameters the environment's chain is running under.
    pub fn get_network_config(&self) -> NetworkConfig {
        self.db.get_network_config()
    }

    /// Verify `tx`'s auth claims; see
    /// [`TestEnvironment::verify_signed_transaction_auth`].
    pub fn run_auth_checks(
        &self,
        tx: &SignedTransaction,
    ) -> thruput_xdr::auth_claim::AuthVerifyErrorCode {
        let serialized_tx = txs::encode(tx).unwrap();
        let ffi_serialized_tx = thruput_sys::FFISerializedSignedTransaction {
            data: serialized_tx.as_ptr() as *mut u8,
            len: serialized_tx.len().try_into().unwrap(),
        };
        let res = unsafe {
            thruput_sys::verify_signed_transaction_auth_from_db(
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
        Self::new(
            None,
            &test_network_hash(),
            &mock_genesis_root_contract(),
            None,
        )
    }
}

/// Shared handle to a [`TestEnvironment`]; returned by its constructors.
pub type TestEnvironmentHandle = Rc<TestEnvironment>;

/// The genesis root installed by the default constructors: a mock contract
/// whose id sits at the top of the range, away from the low ids tests
/// typically register. The test environment executes contract bodies
/// natively through the mock trampoline, so the genesis root's code never
/// runs — only its deployment exists. Use
/// [`TestEnvironment::with_genesis_root_contract`] to install a different
/// contract (e.g. a real rv32 artifact).
pub fn mock_genesis_root_contract() -> Contract {
    Contract::kMockTest {
        mock_contract_id: u32::MAX as i32,
    }
}

impl TestEnvironment {
    /// Creates an environment on the default test network
    /// ([`test_network_hash`]).
    pub fn new(
        cost_model: Option<&StorageCostModel>,
    ) -> TestEnvironmentHandle {
        Self::with_network_hash(cost_model, &test_network_hash())
    }

    /// Creates an environment on a caller-chosen network, with
    /// [`mock_genesis_root_contract`] as the genesis root.
    pub fn with_network_hash(
        cost_model: Option<&StorageCostModel>,
        network_hash: &NetworkHash,
    ) -> TestEnvironmentHandle {
        Self::with_genesis_root_contract(
            cost_model,
            network_hash,
            &mock_genesis_root_contract(),
        )
    }

    /// Creates an environment installing a caller-chosen genesis root
    /// contract instead of [`mock_genesis_root_contract`].
    pub fn with_genesis_root_contract(
        cost_model: Option<&StorageCostModel>,
        network_hash: &NetworkHash,
        genesis_root_contract: &Contract,
    ) -> TestEnvironmentHandle {
        Self::from_parts(cost_model, network_hash, genesis_root_contract, None)
    }

    /// Start configuring an environment that needs settings the plain
    /// constructors do not reach -- notably the chain's network config
    /// parameters.
    ///
    /// Returns a [`TestEnvironmentBuilder`]; chain `with_*` methods, then
    /// finish with [`build`](TestEnvironmentBuilder::build):
    ///
    /// ```no_run
    /// use thruput_native_exec::test_environment::TestEnvironment;
    ///
    /// let env = TestEnvironment::builder()
    ///     .no_cost()
    ///     .with_max_sha256_buffers(2)
    ///     .build();
    /// ```
    pub fn builder() -> TestEnvironmentBuilder {
        TestEnvironmentBuilder::new()
    }

    /// Every constructor's common path: the one place an environment's
    /// database and runtime context are created. `network_config` is
    /// `None` for the stock execution parameters.
    pub(crate) fn from_parts(
        cost_model: Option<&StorageCostModel>,
        network_hash: &NetworkHash,
        genesis_root_contract: &Contract,
        network_config: Option<&NetworkConfig>,
    ) -> TestEnvironmentHandle {
        let te = Rc::new(Self {
            inner: RefCell::new(TestEnvironmentInner::new(
                cost_model,
                network_hash,
                genesis_root_contract,
                network_config,
            )),
        });

        if std::env::var("SDK_TEST_ENABLE_SYSCALL_LOGGING").is_ok() {
            Self::enable_syscall_logging();
        }

        te
    }

    /// Environment with the chain-default cost model on the default test
    /// network. Not the `Default` trait: constructors return a handle.
    #[allow(clippy::should_implement_trait)]
    pub fn default() -> TestEnvironmentHandle {
        Self::new(None)
    }

    /// Environment in which every storage operation costs zero.
    pub fn no_cost() -> TestEnvironmentHandle {
        Self::new(Some(
            &thruput_storage_cost::StorageCostModel::no_cost_model(),
        ))
    }

    /// Turn on FFI-level syscall logging to stderr for environments
    /// created after the call.
    pub fn enable_syscall_logging() {
        TestEnvironmentInner::enable_syscall_logging(
            thruput_sys::FFILogLevel::Trace,
        );
    }

    /// Advance to the next block, making committed effects visible to
    /// subsequent transactions. Panics if a transaction executed but was
    /// neither committed nor aborted.
    ///
    /// The new block takes its timepoint from the wall clock, so
    /// `thruput_sdk::block::get_block_timepoint` reports a plausible
    /// present-day value that moves between blocks. Use `advance_block_at` to
    /// drive contract logic keyed on time -- deadlines, expiry, vesting, rate
    /// limits.
    ///
    /// The SDK path is a plain code span, not an intra-doc link:
    /// `thruput-sdk` depends on this crate, not the other way round, so there
    /// is nothing here to resolve it against.
    pub fn advance_block(self: &TestEnvironmentHandle) {
        self.inner.borrow_mut().advance_block()
    }

    /// Advance to the next block with its timepoint pinned to
    /// `block_timepoint` (Unix nanoseconds) rather than read from the clock.
    ///
    /// Blocks only move forward in time, so this panics if `block_timepoint`
    /// precedes the current block's. Note the environment opens its first
    /// block on the wall clock, so pin forward from the current
    /// `thruput_sdk::block::get_block_timepoint` rather than at an absolute
    /// past date.
    pub fn advance_block_at(
        self: &TestEnvironmentHandle,
        block_timepoint: TimePoint,
    ) {
        self.inner.borrow_mut().advance_block_at(block_timepoint)
    }

    /// The storage cost model the environment's database is running with.
    pub fn get_current_cost_model(
        self: &TestEnvironmentHandle,
    ) -> StorageCostModel {
        self.inner.borrow().get_current_cost_model()
    }

    /// The execution parameters the environment's chain is running under
    /// (e.g. `max_sha256_buffers`, the VM's live hash-stream cap).
    pub fn get_network_config(self: &TestEnvironmentHandle) -> NetworkConfig {
        self.inner.borrow().get_network_config()
    }

    /// Read a committed contract-data item from the current block snapshot
    /// (`None` if the key holds nothing).
    pub fn get_snapshot_storage_item(
        self: &TestEnvironmentHandle,
        addr: &ContractAddress,
        key: &StorageKeyContractData,
    ) -> Option<WireStorageItem> {
        self.inner.borrow().db.get_snapshot_storage_item(addr, key)
    }

    /// Read an address's committed [`AddressAuthData`] (its registered keys
    /// and required weight) from the current block snapshot (`None` if the
    /// address has none).
    pub fn get_snapshot_auth_data(
        self: &TestEnvironmentHandle,
        addr: &ContractAddress,
    ) -> Option<AddressAuthData> {
        self.inner.borrow().db.get_snapshot_auth_data(addr)
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

    /// Start configuring a transaction to execute in this environment.
    ///
    /// Returns a [`TransactionBuilder`]; chain `with_*` methods (e.g.
    /// [`with_auth_claim`](TransactionBuilder::with_auth_claim)) to customize
    /// the mocked transaction context, then finish with
    /// [`transact`](TransactionBuilder::transact),
    /// [`transact_manual`](TransactionBuilder::transact_manual), or
    /// [`build`](TransactionBuilder::build).
    ///
    /// ```ignore
    /// env.transaction()
    ///     .with_auth_claim(my_claim)
    ///     .transact(|tx| tx.call(&addr).run(|| { /* ... */ }));
    /// ```
    pub fn transaction(self: &TestEnvironmentHandle) -> TransactionBuilder {
        TransactionBuilder::new(self.clone())
    }

    /// Execute a transaction with auto-commit and the default configuration.
    ///
    /// Creates a MockTransactionHandler, gives the closure a mutable reference,
    /// and always calls try_commit_transaction when the closure returns
    /// (hogd handles abort internally when the root frame reverted).
    ///
    /// The transaction has exactly one root frame: a second `tx.call`
    /// inside the closure panics; use a child frame or another `transact`.
    ///
    /// Shorthand for `env.transaction().transact(f)`; use
    /// [`transaction`](Self::transaction) to customize the transaction first.
    #[must_use]
    pub fn transact(
        self: &TestEnvironmentHandle,
        f: impl FnOnce(&mut MockTransactionHandler) -> FrameResult,
    ) -> TransactResult {
        self.transaction().transact(f)
    }

    /// Execute a transaction with manual commit/abort control and the default
    /// configuration.
    ///
    /// The closure receives a MockTransactionHandler and is responsible for
    /// calling tx.try_commit_transaction() or tx.abort_transaction().
    /// The closure's return value is forwarded as the return value of transact_manual.
    ///
    /// Shorthand for `env.transaction().transact_manual(f)`; use
    /// [`transaction`](Self::transaction) to customize the transaction first.
    pub fn transact_manual<R>(
        self: &TestEnvironmentHandle,
        f: impl FnOnce(&mut MockTransactionHandler) -> R,
    ) -> R {
        self.transaction().transact_manual(f)
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
    ) -> thruput_xdr::auth_claim::AuthVerifyErrorCode {
        self.inner.borrow().run_auth_checks(tx)
    }

    /// Check the auth claims on the provided transaction
    /// against the provided source and auxiliary auth data.
    /// Does not load any information from the database.
    ///
    /// Pass `None` to represent "no registered auth data" (the pubkey-claim
    /// case). Pass `Some(d)` to supply serialized [`AddressAuthData`].
    ///
    /// `network_hash` is part of every signature preimage, so it must be the
    /// network the signatures were produced for — see
    /// `thruput_domain_separator::transaction_signature_preimage`. There is
    /// no database here to read it from, so it is passed explicitly;
    /// [`test_network_hash`] is what a [`TestEnvironment`] would use.
    pub fn verify_signed_transaction_auth_static(
        tx: &SignedTransaction,
        source_auth_data: Option<&AddressAuthData>,
        network_hash: &NetworkHash,
    ) -> thruput_xdr::auth_claim::AuthVerifyErrorCode {
        let serialized_tx = txs::encode(tx).unwrap();
        let ffi_serialized_tx = thruput_sys::FFISerializedSignedTransaction {
            data: serialized_tx.as_ptr() as *mut u8,
            len: serialized_tx.len().try_into().unwrap(),
        };

        // `None` → null pointer with len=0 (pubkey-claim path, 0.20.1+).
        // `Some(d)` → serialize and pass as before.
        let serialized_source_auth_data =
            source_auth_data.map(|d| txs::encode(d).unwrap());
        let ffi_serialized_source_auth_data =
            match &serialized_source_auth_data {
                Some(bytes) => thruput_sys::FFISourceAuthData {
                    data: bytes.as_ptr() as *mut u8,
                    len: bytes.len().try_into().unwrap(),
                },
                None => thruput_sys::FFISourceAuthData {
                    data: core::ptr::null_mut(),
                    len: 0,
                },
            };

        let res = unsafe {
            thruput_sys::verify_signed_transaction_auth(
                ffi_serialized_tx,
                ffi_serialized_source_auth_data,
                network_hash.as_ptr(),
            )
        };
        let err: FFIErrorCode = res.ffi_error_code.into();
        if err != FFIErrorCode::None {
            panic!("failed to run auth checks: {:?}", err);
        }
        res.auth_error_code.into()
    }
}

fn deployment_contract_size(contract: &Contract) -> u32 {
    match contract {
        Contract::kNative { .. } => 0,
        Contract::kMockTest { .. } => TEST_CONTRACT_SIZE,
        Contract::kRv32 { rv32_contract } => rv32_contract.data.len() as u32,
        other => panic!("unsupported contract variant: {:?}", other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use xdrs::BytesM;

    const TEST_ADDR: ContractAddress = ContractAddress::new([0x11; 32]);

    #[test]
    fn test_network_hash_is_sha256_of_the_test_network_name() {
        // Pinned so the network every test runs on cannot drift silently:
        // sha256("LOCAL_UNIT_TEST").
        let expected: [u8; 32] = [
            0xb0, 0xb2, 0xfc, 0xc8, 0x57, 0xda, 0xa6, 0x04, 0x78, 0x7b, 0xb8,
            0x4b, 0x45, 0xa4, 0xca, 0xf0, 0x32, 0xea, 0x6b, 0x93, 0xf5, 0xa5,
            0xed, 0x1c, 0x14, 0xa9, 0xbf, 0xf8, 0xfa, 0x65, 0xbb, 0x52,
        ];
        assert_eq!(test_network_hash(), NetworkHash::new(expected.into()));
    }

    #[test]
    fn deployment_contract_size_uses_rv32_blob_length() {
        let contract = Contract::kRv32 {
            rv32_contract: ContractBlob {
                data: BytesM(vec![1, 2, 3, 4, 5]),
            },
        };

        assert_eq!(deployment_contract_size(&contract), 5);
    }

    #[test]
    fn get_snapshot_storage_item_reads_committed_bytestring() {
        let env = TestEnvironment::new(None);
        let key = StorageKeyContractData::new([0x22; 32]);
        env.setup_grant_storage_tokens(&TEST_ADDR, 1_000_000_000);

        let result = env.transact(|tx| {
            tx.call(&TEST_ADDR).run(|| {
                thruput_xdr::syscalls::raw_BytestringSet(&key, b"hello")
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
        // Any created, deployable contract works: the transact below runs
        // through the mock trampoline regardless of the deployed type, and
        // native precompiles are banned from redeployment (hogd #3592).
        // A fresh environment hands out mock contract ids from zero.
        let _mock = env.inner.borrow_mut().add_contract();
        let contract = thruput_xdr::contract::Contract::kMockTest {
            mock_contract_id: 0,
        };
        env.inner.borrow_mut().setup_create_contract(&contract);
        env.inner
            .borrow_mut()
            .setup_create_deployment(&TEST_ADDR, &contract);

        let result = env.transact(|tx| {
            tx.call(&TEST_ADDR).run(|| {
                thruput_xdr::syscalls::raw_BytestringSet(&key, b"hello")
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
