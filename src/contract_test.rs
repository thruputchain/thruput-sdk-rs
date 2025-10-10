#[macro_export]
macro_rules! contract_test {
    ($name:ident $test_body:tt) => {
        #[test]
        fn $name() {
            let mut env = sdk::TestEnvironment::new();

            let test_contract = {
                let mut contract = env.add_contract();
                let method = sdk::contract::ContractEntrypointID::from(1u64.to_be_bytes());
                contract.register_method(method, || -> () {
                    sdk::handle::handle_call(|()| {
                        $test_body
                        return ();
                    });
                });
                contract.as_contract()
            };

            let contract_hash = env.setup_create_contract(&test_contract);

            let mut handler = env.prepare_test(move || -> () {
                let nonce = sdk::types::SHA256Hash::from([7; 32]);
                let deployed_addr =
                    sdk::deploy::deploy_contract(&nonce, &contract_hash)
                        .unwrap();

                // Invoke contract method 1
                let method = sdk::contract::ContractEntrypointID::from(1u64.to_be_bytes());
                sdk::invoke::invoke::<(), ()>(
                    &deployed_addr,
                    method,
                    u64::MAX,
                    &(),
                ).unwrap_or_else(|e| panic!("contract test body failed: {}", e));
            });

            let res = handler.root_invoke();
            if res.result.panic != sdk::InvokeErrorCode::kInvokeNone as i32 {
                panic!("root invocation panicked with code {}", res.result.panic);
            }
        }
    };
}

mod contract_test_wasm {
    use alloc::boxed::Box;
    use crate::SHA256Hash;
    use core::error::Error;

    pub trait ContractTest {
        fn run(&self, contract_hash: &SHA256Hash) -> Result<(), Box<dyn Error>>;
    }
}

pub use contract_test_wasm::*;

pub fn test_wasm<P, T>(path: P, test: T)
    where
        P: AsRef<std::path::Path>,
        T: ContractTest + Clone + 'static + core::panic::UnwindSafe,
{
    use crate::*;
    use crate::contract::ContractBlob;
    use crate::TestEnvironment;

    let env = TestEnvironment::new();
    TestEnvironment::enable_syscall_logging();

    // Load the wasm and setup the contract
    let wasm = std::fs::read(path.as_ref())
        .map(BytesM)
        .unwrap_or_else(|e| panic!("couldn't load wasm to test: {}", e));
    let contract: ContractBlob = ContractBlob { wasm };
    let contract = Contract::kWasmi {
        wasmi_contract: contract,
    };
    let contract_hash = env.setup_create_contract(&contract);

    let mut handler = env.prepare_test(move || -> () {
        test.run(&contract_hash).unwrap_or_else(|e|
            panic!("contract test failed: {}", e));
    });

    let res = handler.root_invoke();

    match res.result.panic.try_into() {
        Ok(InvokeErrorCode::kInvokeNone) => { /* success */ },
        Ok(invoke_err) => panic!("test-invoke failed ({:?})", invoke_err),
        Err(_) => panic!("test-invoke failed with unknown error ({})", res.result.panic),
    }
}
