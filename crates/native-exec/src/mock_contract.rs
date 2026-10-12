use crate::test_environment::TestEnvironment;

use crate::invoke_error::invoke_wrapper;
use sha2::{Digest, Sha256};
use std::boxed::Box;
use std::collections::HashMap;
use std::panic::UnwindSafe;
use std::rc::Rc;
use thruput_xdr::contract::Contract;
use thruput_xdr::contract::ContractEntrypointID;
use thruput_xdr::types::ContractAddress;
use thruput_xdr::types::SHA256Hash;
use xdrs::txs;

/// Backing storage for a mock contract, owned by the [`TestEnvironment`];
/// interact with it through [`MockContract`] handles.
pub struct MockContractInner {
    // implicitly, this contract is Contract::kMockTest { mock_contract_id : index }
    index: i32,
    hash: SHA256Hash, // sha256(Contract::kMocktest(index))
    funcs: HashMap<
        ContractEntrypointID,
        Box<dyn Fn() -> thruput_sys::ForeignInvokeResult + Send + Sync>,
    >,
    /// Run instead of a deterministic failure when an invoked method id
    /// matches no entry in `funcs`. `None` (the default) keeps the
    /// historical behavior; `sdk::register_mock_package` installs a
    /// `LibraryExn::UnknownMethod` throw, so a mock standing in for a real
    /// contract agrees with the rv32 guest dispatcher.
    unknown: Option<
        Box<dyn Fn() -> thruput_sys::ForeignInvokeResult + Send + Sync>,
    >,
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
    ///
    /// Panics if `id` is already registered on this mock: two methods
    /// resolving to one dispatch ID means the second registration would
    /// silently shadow the first, so duplicate definitions (same interface
    /// name, major version, and method name) fail loudly at test setup
    /// instead. There is deliberately no replace/override variant; on-chain
    /// dispatch has no override either.
    ///
    /// `Send + Sync` on `func` is what keeps a mock body safe to run on the
    /// thread the coroutine resumed on rather than the one that registered it;
    /// see [`invoke_wrapper`]. Capturing `Rc`/`RefCell` state to record calls
    /// is rejected here -- use `Arc`/`Mutex`, or an `AtomicUsize` for a plain
    /// counter.
    pub fn register_method<F>(&mut self, id: ContractEntrypointID, func: F)
    where
        F: Fn() + UnwindSafe + Clone + Send + Sync + 'static,
    {
        // Entry API so the rejected registration never touches the map: a
        // caller that catches the panic must still observe the ORIGINAL
        // handler on later dispatches, not the rejected replacement.
        match unsafe { &mut (*self.contract).funcs }.entry(id) {
            std::collections::hash_map::Entry::Occupied(_) => panic!(
                "duplicate entrypoint registration for dispatch ID \
                 {:016X}: two contract methods share one entrypoint ID \
                 (same interface name, major version, and method name), so \
                 calls would silently route to only one of them. Remove or \
                 rename the duplicate definition.",
                id.into_u64(),
            ),
            std::collections::hash_map::Entry::Vacant(slot) => {
                slot.insert(invoke_wrapper(func));
            }
        }
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
        F: Fn() + UnwindSafe + Clone + Send + Sync + 'static,
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
    ) -> Option<&(dyn Fn() -> thruput_sys::ForeignInvokeResult + Send + Sync)>
    {
        self.funcs.get(&method).map(|boxed| boxed.as_ref())
    }

    /// The handler for undefined method ids, if one was registered.
    pub fn get_unknown_func(
        &self,
    ) -> Option<&(dyn Fn() -> thruput_sys::ForeignInvokeResult + Send + Sync)>
    {
        self.unknown.as_ref().map(|boxed| boxed.as_ref())
    }
}
