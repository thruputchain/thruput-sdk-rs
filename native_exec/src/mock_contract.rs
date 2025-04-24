use crate::test_environment::TestEnvironment;

use crate::bindings::ForeignInvokeResult;
use crate::invoke_error::invoke_wrapper;
use groundhog_xdr::contract::Contract;
use groundhog_xdr::serialize::{Limits, WriteXdr};
use groundhog_xdr::types::SHA256Hash;
use sha2::{Digest, Sha256};
use std::boxed::Box;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::panic::UnwindSafe;

pub struct MockContractInner {
    // implicitly, this contract is Contract::kMockTest { mock_contract_id : index }
    index: u32,
    hash: SHA256Hash,
    funcs: HashMap<u32, Box<dyn Fn() -> ForeignInvokeResult>>,
}

pub struct MockContract<'a> {
    contract: *mut MockContractInner,
    _marker: PhantomData<&'a TestEnvironment>,
}

fn u32_to_contract(index: u32) -> Contract {
    Contract::kMockTest {
        mock_contract_id: index,
    }
}

impl<'a> MockContract<'a> {
    pub fn method_name_from_int(method_id: u32) -> String {
        format!("{:X}", method_id)
    }

    pub fn method_int_from_sdk_name(name: &str) -> u32 {
        let mut hasher = Sha256::new();
        hasher.update(name.as_bytes());
        let result = hasher.finalize();

        let prefix = &result[0..4];
        let num: u32 = u32::from_be_bytes(prefix.try_into().unwrap());
        num
    }

    pub fn as_contract(&self) -> Contract {
        u32_to_contract(unsafe { (*self.contract).index })
    }

    pub fn as_hash(&self) -> SHA256Hash {
        unsafe { (*self.contract).hash }
    }

    pub fn register_method(self: &mut Self, id: u32, func: fn() -> i32) {
        unsafe { (*self.contract).funcs.insert(id, invoke_wrapper(func)) };
    }

    pub fn register_func<F>(self: &mut Self, id: u32, func: F)
    where
        F: Fn() -> i32 + UnwindSafe + Clone + 'static,
    {
        unsafe {
            (*self.contract)
                .funcs
                .insert(id, invoke_wrapper(func.clone()))
        };
    }
}

impl MockContractInner {
    pub fn new(n: u32) -> Self {
        let mut hasher = Sha256::new();
        let contract = u32_to_contract(n);
        hasher.update(contract.to_xdr(Limits::none()).unwrap());
        let result = hasher.finalize();
        Self {
            index: n,
            hash: result.into(),
            funcs: HashMap::new(),
        }
    }

    pub fn as_hash(&self) -> SHA256Hash {
        self.hash
    }
}
