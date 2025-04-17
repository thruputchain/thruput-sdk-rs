use core::ffi::c_void;

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

use groundhog_xdr::contract::Contract;
use groundhog_xdr::ffi_enums::InvokeErrorCode;
use groundhog_xdr::transaction::SignedTransaction;
use groundhog_xdr::types::ContractAddress;
use groundhog_xdr::types::SHA256Hash;
use groundhog_xdr::{ByteArray, Limits, WriteXdr};

use sha2::Digest;
use sha2::Sha256;

use std::collections::HashMap;
use std::marker::PhantomData;
use std::panic::UnwindSafe;

use std::panic;
#[cfg(test)]
use std::panic::panic_any;

type InvokeResult = ForeignInvokeResult;

impl PartialEq for ForeignInvokeResult {
    fn eq(&self, other: &Self) -> bool {
        (self.result_code == other.result_code) && (self.panic == other.panic)
    }
}

use crate::invoke_error::set_override_panic_hook;
use crate::invoke_error::InvokeError;

pub unsafe extern "C" fn trampoline(
    context: *mut c_void,
    method: u32,
) -> InvokeResult {
    let c: *mut MockContractInner = unsafe { core::mem::transmute(context) };

    match unsafe { (*c).funcs.get(&method) } {
        Some(x) => {
            return x();
        }
        None => {
            return InvokeResult {
                result_code: 0,
                panic: InvokeErrorCode::kDeterministic as i32,
            }
        }
    }
}

pub fn invoke_wrapper<'a, F>(wrapped: F) -> Box<dyn Fn() -> InvokeResult + 'a>
where
    F: Fn() -> i32 + UnwindSafe + Clone + 'a,
{
    Box::new(move || {
        let w = wrapped.clone();
        // This wrapper never returns InvokeErrorCode::kOutOfGas.
        let result = panic::catch_unwind(move || w());

        match result {
            Ok(result_code) => {
                return InvokeResult {
                    result_code: result_code,
                    panic: InvokeErrorCode::kNone as i32,
                }
            }
            Err(err) => {
                if let Some(my_error) = err.downcast_ref::<InvokeError>() {
                    return InvokeResult {
                        result_code: my_error.error_code,
                        panic: my_error.error_type.clone() as i32,
                    };
                }
                return InvokeResult {
                    result_code: 0,
                    panic: InvokeErrorCode::kUnrecoverable as i32,
                };
            }
        }
    })
}

struct MockContractInner {
    index: u32, // this is passed as the raw bytes in the contract, and thus identified as sha256(index).
    hash: SHA256Hash,
    funcs: HashMap<u32, Box<dyn Fn() -> InvokeResult>>,
}

// TODO(geoff): I cannot figure out how to prevent rust from moving objects
// without just converting back to raw pointers.
// Pin<Box<T>> with PhantomPinned still wound up getting moved, somehow.
// It would be nice to avoid this later of indirection, as that would make passing
// (or at least seem to make) passing closures in as parameters to test functions
// much easier.
pub struct TestEnvironment {
    db: *mut c_void,
    contract_context: *mut c_void,
    mock_contract_index: u32,
    contracts: Vec<*mut MockContractInner>, // owns these contracts
}

pub struct MockContract<'a> {
    contract: *mut MockContractInner,
    _marker: PhantomData<&'a TestEnvironment>,
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

    pub fn setup_create_contract(&mut self, contract: &Contract) -> SHA256Hash {
        let contract_bytes = contract.to_xdr(Limits::none()).unwrap();
        unsafe {
            setup_create_contract(
                self.db,
                contract_bytes.as_ptr(),
                contract_bytes.len().try_into().unwrap(),
            )
        };
        let mut hasher = Sha256::new();
        hasher.update(&contract_bytes);
        ByteArray::new(hasher.finalize().into())
    }

    pub fn setup_create_deployment(
        &mut self,
        addr: &ContractAddress,
        contract: &Contract,
    ) {
        let contract_bytes = contract.to_xdr(Limits::none()).unwrap();

        let mut hasher = Sha256::new();
        hasher.update(&contract_bytes);
        let contract_hash: SHA256Hash =
            ByteArray::new(hasher.finalize().into());

        unsafe {
            setup_create_deployment(
                self.db,
                addr.as_ptr(),
                contract_hash.as_ptr(),
            )
        };
    }

    pub fn advance_block(&mut self) {
        unsafe { advance_new_block(self.db) }
    }

    pub fn add_contract(&mut self) -> MockContract<'a> {
        let out: *mut MockContractInner = Box::into_raw(Box::new(
            MockContractInner::new(self.mock_contract_index),
        ));

        self.contracts.push(out);
        self.mock_contract_index += 1;

        let res = unsafe {
            add_contract_and_method(
                self.contract_context,
                (*out).as_hash().as_ptr(),
                Some(trampoline),
                out as *mut c_void,
            )
        };
        if res != 0 {
            panic!("failed to link fn");
        }
        MockContract {
            contract: out,
            _marker: PhantomData,
        }
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

        use groundhog_xdr::transaction::*;

        let invoke_addr = ByteArray::new([0xFF; 32]);

        self.setup_create_deployment(
            &invoke_addr,
            &test_contract.as_contract(),
        );

        let stx = SignedTransaction {
            tx: Transaction {
                invocation: Invocation {
                    invoked_address: ByteArray::new([0xFF; 32]),
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

fn u32_to_contract(index: u32) -> Contract {
    let bytes = index.to_xdr(Limits::none()).unwrap();
    Contract {
        contract_data: bytes.try_into().unwrap(),
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
    fn new(n: u32) -> Self {
        let mut hasher = Sha256::new();
        let contract = u32_to_contract(n);
        hasher.update(contract.to_xdr(Limits::none()).unwrap());
        let result = hasher.finalize();
        Self {
            index: n,
            hash: ByteArray::new(result.into()),
            funcs: HashMap::new(),
        }
    }

    pub fn as_hash(&self) -> SHA256Hash {
        self.hash
    }

    // pub fn register_method(self: &mut Self, id: u32, func: fn() -> i32) {
    //    self.funcs.insert(id, invoke_wrapper(func));
    // }
}

// Can run one, and only one, transaction,
// and then get the results
pub struct TransactionHandler<'a> {
    handler: *mut c_void,
    _marker: PhantomData<&'a TestEnvironment>,
    // cannot outlive the environment.
    // TODO(geoff): we could swap the backing pointer to the db
    // to be a shared ptr, and have handler hold a reference,
    // like we do with the mock contract context.
    invoked: bool,
}

impl<'a> TransactionHandler<'a> {
    pub fn new(env: &'a TestEnvironment, stx: &SignedTransaction) -> Self {
        let tx_bytes = stx.to_xdr(Limits::none()).unwrap();
        let out = unsafe {
            Self {
                handler: open_syscall_handler(
                    env.db,
                    env.contract_context,
                    tx_bytes.as_ptr(),
                    tx_bytes.len().try_into().unwrap(),
                ),
                _marker: PhantomData,
                invoked: false,
            }
        };
        if out.handler == core::ptr::null_mut() {
            panic!("failed to initialize handler!");
        }
        out
    }

    pub fn root_invoke(&mut self) -> ForeignRootInvokeResult {
        if self.invoked {
            panic!("should not double invoke");
        }
        self.invoked = true;

        unsafe { root_invoke(self.handler) }
    }
}

impl Drop for TransactionHandler<'_> {
    fn drop(&mut self) {
        unsafe { close_syscall_handler(self.handler) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // There's a race condition on the panic handler,
    // which changes how tests behave.  So run them all sequentially.
    #[test]
    fn test_panic_runner() {
        test_error_wrap();
        test_panic();
        test_panic_ok();
        test_sdk_panic();
        test_option_panic();
        test_pass_invoke_error_through_panic_hook();
    }

    fn test_error_wrap() {
        let _ = panic::take_hook();
        let f: fn() -> i32 = || {
            return 1;
        };

        let boxed = invoke_wrapper(f);

        assert_eq!(
            (*boxed)(),
            InvokeResult {
                result_code: 1,
                panic: 0
            }
        );
    }

    fn test_panic() {
        let _ = panic::take_hook();
        let f: fn() -> i32 = || panic!("unknown");

        let boxed = invoke_wrapper(f);

        assert_eq!(
            (*boxed)(),
            InvokeResult {
                result_code: 0,
                panic: InvokeErrorCode::kUnrecoverable as i32
            }
        );
    }

    fn test_panic_ok() {
        let _ = panic::take_hook();
        let f: fn() -> i32 =
            || panic_any(crate::invoke_error::validation_error());

        let boxed = invoke_wrapper(f);

        assert_eq!(
            (*boxed)(),
            InvokeResult {
                result_code: 0,
                panic: InvokeErrorCode::kValidationFailure as i32
            }
        );
    }

    fn test_sdk_panic() {
        set_override_panic_hook();
        let f: fn() -> i32 = || {
            panic!("something");
        };

        let boxed = invoke_wrapper(f);

        assert_eq!(
            (*boxed)(),
            InvokeResult {
                result_code: 0,
                panic: InvokeErrorCode::kDeterministic as i32
            }
        );
    }

    fn test_option_panic() {
        set_override_panic_hook();
        let f: fn() -> i32 = || {
            let x: Option<i32> = None;
            return x.unwrap();
        };

        let boxed = invoke_wrapper(f);

        assert_eq!(
            (*boxed)(),
            InvokeResult {
                result_code: 0,
                panic: InvokeErrorCode::kDeterministic as i32
            }
        );
    }

    fn test_pass_invoke_error_through_panic_hook() {
        set_override_panic_hook();
        let f: fn() -> i32 =
            || panic_any(crate::invoke_error::validation_error());

        let boxed = invoke_wrapper(f);

        assert_eq!(
            (*boxed)(),
            InvokeResult {
                result_code: 0,
                panic: InvokeErrorCode::kValidationFailure as i32
            }
        );
    }
}
