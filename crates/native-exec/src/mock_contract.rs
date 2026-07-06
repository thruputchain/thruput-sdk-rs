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

pub struct MockContractInner {
    // implicitly, this contract is Contract::kMockTest { mock_contract_id : index }
    index: i32,
    hash: SHA256Hash, // sha256(Contract::kMocktest(index))
    funcs: HashMap<
        ContractEntrypointID,
        Box<dyn Fn() -> groundhog_sys::ForeignInvokeResult>,
    >,
}

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
    pub fn new(
        inner: *mut MockContractInner,
        env: Rc<TestEnvironment>,
    ) -> Self {
        MockContract {
            contract: inner,
            _env: env,
        }
    }

    pub fn as_contract(&self) -> Contract {
        i32_to_contract(unsafe { (*self.contract).index })
    }

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

    pub fn register_method<F>(&mut self, id: ContractEntrypointID, func: F)
    where
        F: Fn() + UnwindSafe + Clone + 'static,
    {
        // Each trampoline invocation of this method is one cross-call
        // frame (its own guest instance in the real system), so measure
        // it as its own allocation scope and log a per-invocation
        // record. The scope closes after invoke_wrapper's internal
        // catch_unwind, so abort/panic paths are measured too.
        let index = unsafe { (*self.contract).index };
        let entrypoint = id;
        let wrapped = invoke_wrapper(func);
        let measured = Box::new(move || {
            let scope = crate::alloc_stats::FrameMeasureScope::open();
            let res = wrapped();
            let stats = scope.close();
            crate::alloc_stats::push_frame_record(
                crate::alloc_stats::FrameAllocRecord {
                    mock_contract_index: index,
                    entrypoint,
                    stats,
                },
            );
            res
        });
        unsafe { (*self.contract).funcs.insert(id, measured) };
    }

    pub fn register<F: FnOnce(&mut Self)>(&mut self, f: F) -> &mut Self {
        f(self);
        self
    }
}

impl MockContractInner {
    pub fn new(n: i32) -> Self {
        let mut hasher = Sha256::new();
        let contract = i32_to_contract(n);
        hasher.update(txs::encode(&contract).unwrap());
        let result: [u8; 32] = hasher.finalize().into();
        Self {
            index: n,
            hash: result.into(),
            funcs: HashMap::new(),
        }
    }

    pub fn as_hash(&self) -> &SHA256Hash {
        &self.hash
    }

    pub fn get_func(
        &self,
        method: ContractEntrypointID,
    ) -> Option<&dyn Fn() -> groundhog_sys::ForeignInvokeResult> {
        self.funcs.get(&method).map(|boxed| boxed.as_ref())
    }
}
