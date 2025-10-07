use crate::test_environment::TestEnvironment;

use crate::bindings::ForeignInvokeResult;
use crate::invoke_error::invoke_wrapper;
use groundhog_xdr::contract::Contract;
use xdrs::serialize_txs;
use groundhog_xdr::contract::ContractEntrypointID;
use groundhog_xdr::types::SHA256Hash;
use sha2::{Digest, Sha256};
use std::boxed::Box;
use std::collections::HashMap;
use std::panic::UnwindSafe;
use std::rc::Rc;

pub struct MockContractInner {
    // implicitly, this contract is Contract::kMockTest { mock_contract_id : index }
    index: i32,
    hash: SHA256Hash, // sha256(Contract::kMocktest(index))
    funcs: HashMap<ContractEntrypointID, Box<dyn Fn() -> ForeignInvokeResult>>,
}

pub struct MockContract {
    contract: *mut MockContractInner,
    env: Rc<TestEnvironment>,
}

fn i32_to_contract(index: i32) -> Contract {
    Contract::kMockTest {
        mock_contract_id: index,
    }
}

impl MockContract {
    pub fn new(inner: *mut MockContractInner, env: Rc<TestEnvironment>) -> Self {
        MockContract {
            contract: inner,
            env: env,
        }
    }

    /*
    pub fn method_name_from_int(method_id: MethodID) -> String {
        format!("{:X}", method_id)
    }

    pub fn method_int_from_sdk_name(name: &str) -> MethodID {
        let mut hasher = Sha256::new();
        hasher.update(name.as_bytes());
        let result = hasher.finalize();

        let prefix = &result[0..8];
        let num: MethodID = MethodID::from_be_bytes(prefix.try_into().unwrap());
        num
    }
    */

    pub fn as_contract(&self) -> Contract {
        i32_to_contract(unsafe { (*self.contract).index })
    }

    pub fn as_hash(&self) -> &SHA256Hash {
        unsafe { &(*self.contract).hash }
    }

    pub fn register_method<F>(self: &mut Self, id: ContractEntrypointID, func: F)
    where
        F: Fn() -> () + UnwindSafe + Clone + 'static,
    {
        unsafe { (*self.contract).funcs.insert(id, invoke_wrapper(func)) };
    }

    pub fn register_func<F>(self: &mut Self, id: ContractEntrypointID, func: F)
    where
        F: Fn() -> () + UnwindSafe + Clone + 'static,
    {
        unsafe {
            (*self.contract)
                .funcs
                .insert(id, invoke_wrapper(func.clone()))
        };
    }
}

impl MockContractInner {
    pub fn new(n: i32) -> Self {
        let mut hasher = Sha256::new();
        let contract = i32_to_contract(n);
        hasher.update(serialize_txs::encode(&contract).unwrap());
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
    ) -> Option<&Box<dyn Fn() -> ForeignInvokeResult>> {
        self.funcs.get(&method)
    }
}
