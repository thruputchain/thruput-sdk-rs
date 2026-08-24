use crate::test_environment::TestEnvironment;

use crate::invoke_error::invoke_wrapper;
use groundhog_xdr::contract::Contract;
use groundhog_xdr::contract::ContractEntrypointID;
use groundhog_xdr::types::ContractAddress;
use groundhog_xdr::types::SHA256Hash;
use sha2::{Digest, Sha256};
use std::boxed::Box;
use std::collections::HashMap;
use std::panic::UnwindSafe;
use std::rc::Rc;
use xdrs::txs;

/// Backing storage for a mock contract, owned by the [`TestEnvironment`];
/// interact with it through [`MockContract`] handles.
pub struct MockContractInner {
    // implicitly, this contract is Contract::kMockTest { mock_contract_id : index }
    index: i32,
    hash: SHA256Hash, // sha256(Contract::kMocktest(index))
    funcs: HashMap<
        ContractEntrypointID,
        Box<dyn Fn() -> groundhog_sys::ForeignInvokeResult>,
    >,
    /// Run instead of a deterministic failure when an invoked method id
    /// matches no entry in `funcs`. `None` (the default) keeps the
    /// historical behavior; `sdk::register_mock_package` installs a
    /// `LibraryExn::UnknownMethod` throw, so a mock standing in for a real
    /// contract agrees with the rv32 guest dispatcher.
    unknown: Option<Box<dyn Fn() -> groundhog_sys::ForeignInvokeResult>>,
}

/// Handle to a mock contract registered in a [`TestEnvironment`]: a
/// contract whose entrypoints are native closures instead of rv32 code.
pub struct MockContract {
    contract: *mut MockContractInner,
    _env: Rc<TestEnvironment>,
}

fn i32_to_contract(index: i32) -> Contract {
    Contract::kMockTest {
        mock_contract_id: index,
    }
}

impl MockContract {
    /// Wrap a raw [`MockContractInner`] owned by `env`.
    pub fn new(
        inner: *mut MockContractInner,
        env: Rc<TestEnvironment>,
    ) -> Self {
        MockContract {
            contract: inner,
            _env: env,
        }
    }

    /// The `Contract::kMockTest` value identifying this mock.
    pub fn as_contract(&self) -> Contract {
        i32_to_contract(unsafe { (*self.contract).index })
    }

    /// The contract hash (`sha256(Contract::kMockTest(index))`).
    pub fn as_hash(&self) -> &SHA256Hash {
        unsafe { &(*self.contract).hash }
    }

    /// Deploy this prepared mock contract at a fixed address in test setup.
    ///
    /// This uses the test environment's setup path, so it should be called
    /// before executing the transaction under test.
    pub fn deploy_contract_to(&self, addr: &ContractAddress) {
        self._env.setup_create_deployment(addr, &self.as_contract());
    }

    /// Register `func` as the entrypoint for method `id`. The closure runs
    /// inside the frame's catch site, so panics and aborts out of it map to
    /// frame outcomes.
    pub fn register_method<F>(&mut self, id: ContractEntrypointID, func: F)
    where
        F: Fn() + UnwindSafe + Clone + 'static,
    {
        unsafe { (*self.contract).funcs.insert(id, invoke_wrapper(func)) };
    }

    /// Install the handler for method ids this contract does not define,
    /// replacing any previous one.
    ///
    /// The registered closure runs inside the same `invoke_wrapper` catch
    /// site as a real method, so aborting out of it (e.g. via
    /// `sdk::exn::library_throw`) reaches the caller as an ordinary
    /// exception rather than escaping the frame.
    pub fn register_unknown_method<F>(&mut self, func: F)
    where
        F: Fn() + UnwindSafe + Clone + 'static,
    {
        unsafe { (*self.contract).unknown = Some(invoke_wrapper(func)) };
    }

    /// Apply a registrar closure (e.g. a generated `__mock` function),
    /// for chaining.
    pub fn register<F: FnOnce(&mut Self)>(&mut self, f: F) -> &mut Self {
        f(self);
        self
    }
}

impl MockContractInner {
    /// Create the backing storage for mock contract number `n`.
    pub fn new(n: i32) -> Self {
        let mut hasher = Sha256::new();
        let contract = i32_to_contract(n);
        hasher.update(txs::encode(&contract).unwrap());
        let result: [u8; 32] = hasher.finalize().into();
        Self {
            index: n,
            hash: result.into(),
            funcs: HashMap::new(),
            unknown: None,
        }
    }

    /// The contract hash (`sha256(Contract::kMockTest(index))`).
    pub fn as_hash(&self) -> &SHA256Hash {
        &self.hash
    }

    /// The registered entrypoint for `method`, if any.
    pub fn get_func(
        &self,
        method: ContractEntrypointID,
    ) -> Option<&dyn Fn() -> groundhog_sys::ForeignInvokeResult> {
        self.funcs.get(&method).map(|boxed| boxed.as_ref())
    }

    /// The handler for undefined method ids, if one was registered.
    pub fn get_unknown_func(
        &self,
    ) -> Option<&dyn Fn() -> groundhog_sys::ForeignInvokeResult> {
        self.unknown.as_ref().map(|boxed| boxed.as_ref())
    }
}
