use crate::{BytesM, Contract, InvokeErrorCode, TestEnvironment};

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
                    sdk::contract_test_handle::handle_call(|()| {
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

#[macro_export]
macro_rules! contract_test_env {
    (
        fn env_hook($env:ident, $handler:ident) $env_hook:tt
        fn $name:ident() $test_body:tt
        fn post_hook($handler2:ident) $post_hook:tt
    ) => {

        #[test]
        fn $name() {
            let mut env = sdk::TestEnvironment::new();

            let test_contract = {
                let mut contract = env.add_contract();
                let method = sdk::contract::ContractEntrypointID::from(1u64.to_be_bytes());
                contract.register_method(method, || -> () {
                    sdk::contract_test_handle::handle_call(|()| {
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

            fn env_hook(
                $env: &std::rc::Rc<sdk::TestEnvironment>,
                $handler: &mut native_exec::transaction_handler::TransactionHandler,
            ) -> () $env_hook

            env_hook(&mut env, &mut handler);

            let res = handler.root_invoke();
            if res.result.panic != sdk::InvokeErrorCode::kInvokeNone as i32 {
                panic!("root invocation panicked with code {}", res.result.panic);
            }

            let commit_res = handler.try_commit_transaction();
            assert!(commit_res);

            fn post_hook(
                $handler2: &mut native_exec::transaction_handler::TransactionHandler,
            ) -> () $post_hook

            post_hook(&mut handler);
        }
    };
}

mod contract_test_wasm {
    use crate::SHA256Hash;
    use alloc::boxed::Box;
    use core::error::Error;

    pub trait ContractTest {
        fn run(
            &self,
            contract_hash: &SHA256Hash,
        ) -> Result<(), Box<dyn Error>>;
    }
}

pub use contract_test_wasm::*;

#[derive(Copy, Clone, Debug)]
pub struct Stats {
    pub gas_consumed: u64,
}

pub fn test_wasm<P, T>(path: P, test: T) -> Stats
where
    P: AsRef<std::path::Path>,
    T: ContractTest + Clone + 'static + core::panic::UnwindSafe,
{
    let contract = load_wasm_contract(path);

    // Set up the contract
    let env = TestEnvironment::new();
    TestEnvironment::enable_syscall_logging();
    let contract_hash = env.setup_create_contract(&contract);

    let mut handler = env.prepare_test(move || -> () {
        let _ = test
            .run(&contract_hash)
            .unwrap_or_else(|e| panic!("run method failed: {}", e));
    });

    let res = handler.root_invoke();

    match res.result.panic.try_into() {
        Ok(InvokeErrorCode::kInvokeNone) => { /* success */ }
        Ok(invoke_err) => panic!("test-invoke failed ({:?})", invoke_err),
        Err(_) => panic!(
            "test-invoke failed with unknown error ({})",
            res.result.panic
        ),
    }

    let gas_consumed = res.gas_consumed;

    // std::println!("gas consumed: {}", gas_consumed);

    Stats { gas_consumed }
}

pub fn load_wasm_contract<P>(path: P) -> Contract
where
    P: AsRef<std::path::Path>,
{
    use crate::contract::ContractBlob;

    let wasm = std::fs::read(path.as_ref())
        .map(BytesM)
        .unwrap_or_else(|e| {
            panic!(
                "{}: couldn't load wasm: {}",
                path.as_ref().to_str().unwrap_or("?"),
                e
            )
        });

    let contract: ContractBlob = ContractBlob { wasm };

    Contract::kWasmi {
        wasmi_contract: contract,
    }
}

#[allow(unused)]
fn timed<R, F: FnOnce() -> R>(f: F) -> (Stats, R) {
    let gas_start = crate::stats::gas_remaining();
    let result = f();
    let gas_end = crate::stats::gas_remaining();

    if gas_end > gas_start {
        panic!("consumed negative {} gas", gas_end - gas_start);
    }

    let gas_consumed = gas_start.checked_sub(gas_end).unwrap_or_else(|| {
        panic!("consumed negative {} gas", gas_end - gas_start)
    });

    std::println!(
        "Gas start ({}) - end ({}) = consumed ({})",
        gas_start,
        gas_end,
        gas_consumed
    );

    let stats = Stats { gas_consumed };

    (stats, result)
}
