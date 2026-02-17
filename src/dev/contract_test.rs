use crate::*;
use groundhog_xdr::transaction::InvocationFrameStatusCode;
use std::panic::UnwindSafe;
use std::sync::{Arc, Mutex};
use std::vec::Vec;
use groundhog_native_exec as native_exec;
use core::convert::Infallible;
use crate::call::{try_call, CallResult};

pub static TEST_NONCE: SHA256Hash = SHA256Hash::new(hash_to_array!("CONTRACT_TEST_NONCE"));

/// Run some code inside a mock contract.
/// Panic if the invocation or transaction fails.
///
pub fn contract_test_succeeding<F>(env: TestEnvironmentHandle, f: F)
where
    F: Fn() -> () + UnwindSafe + Clone + 'static,
{
    let mut handler = env.prepare_test(f);

    grant_storage_tokens(&env, &handler);

    let res = handler.root_invoke();

    if res.output.status_code()
        != InvocationFrameStatusCode::kInvocationFrameStatusCodeReturn as i32
    {
        panic!("Root invocation panicked with code {:?}", res.output);
    }

    match handler.try_commit_transaction() {
        ExecResult::ExecResultOK => {}
        r => panic!("Unexpected transaction result: {:?}", r),
    }
}

/// Deploy a mock contract with the given code, and invoke it from another mock
/// contract.
///
/// (This allows us to get the CallResult, something I haven't figured out
/// how to do directly from the TestEnvironment.)
///
/// TODO: Support specifying return type and exception type.
///
pub fn contract_test_invoke<F>(
    env: TestEnvironmentHandle,
    f: F,
) -> CallResult<()>
where
    F: Fn() + UnwindSafe + Clone + 'static,
{
    let entrypoint = ContractEntrypointID::from(1u64.to_be_bytes());

    let test_contract = {
        let mut contract = env.add_contract();
        contract.register_method(entrypoint, f);
        contract.as_contract()
    };

    let contract_hash = env.setup_create_contract(&test_contract);

    // It's currently difficult to extract state from mock contracts,
    // so we'll allocate shared storage for the data we want to inspect
    // after the test run:
    //
    let call_result: Arc<Mutex<Option<CallResult>>> =
        Arc::new(Mutex::new(None));

    let mut handler = {
        let call_result = call_result.clone();

        env.prepare_test(move || -> () {
            let deployed_addr = crate::deploy_contract(&TEST_NONCE, &contract_hash);

            type ARGS = ();
            type RET = ();
            type EXN = Infallible;
            let cr = try_call::<ARGS, RET, EXN>(
                &deployed_addr, entrypoint, u64::MAX, &()
            );

            *call_result.lock().unwrap() = Some(cr);
        })
    };

    grant_storage_tokens(&env, &handler);

    let res = handler.root_invoke();
    if res.output.status_code()
        != InvocationFrameStatusCode::kInvocationFrameStatusCodeReturn as i32
    {
        panic!("Root invocation panicked with code {:?}", res.output);
    }

    match handler.try_commit_transaction() {
        ExecResult::ExecResultOK => {}
        // Note: these results are also deterministic:
        //     ExecResultTxDeterministicError
        //     ExecResultTxRootFrameReverted
        r => panic!("Transaction failed: {:?}", r),
    }

    call_result.lock().unwrap().take().unwrap()
}

#[macro_export]
macro_rules! contract_test {
    ($name:ident $test_body:tt) => {
        #[test]
        fn $name() {
            let mut env = $crate::TestEnvironment::new(Some(&$crate::StorageCostModel::no_cost_model()));

            $crate::contract_test::contract_test_succeeding(env, || {
                $test_body
            });
        }
    };
}

// TODO: remove in favor of something like above functions (tests/events.rs is the only user?)
#[macro_export]
macro_rules! contract_test_env {
    (
        fn env_hook($env:ident, $handler:ident) $env_hook:tt
        fn $name:ident() $test_body:tt
        fn post_hook($handler2:ident) $post_hook:tt
    ) => {

        #[test]
        fn $name() {
            let mut env = $crate::TestEnvironment::new(Some(&$crate::StorageCostModel::no_cost_model()));

            let test_contract = {
                let mut contract = env.add_contract();
                let method = $crate::contract::ContractEntrypointID::from(1u64.to_be_bytes());
                contract.register_method(method, || -> () {
                    $crate::dev::internal::handle_call(|()| {
                        $test_body
                        return ();
                    });
                });
                contract.as_contract()
            };

            let contract_hash = env.setup_create_contract(&test_contract);

            let mut handler = env.prepare_test(move || -> () {
                let deployed_addr =
                    $crate::deploy::deploy_contract(&TEST_NONCE, &contract_hash)
                        .unwrap();

                // Invoke contract method 1
                let method = $crate::contract::ContractEntrypointID::from(1u64.to_be_bytes());

                type ARG = ();
                type RET = ();
                type EXN = Infallible;

                $crate::call::call::<ARG, RET, EXN>(
                    &deployed_addr,
                    method,
                    u64::MAX,
                    &(),
                ).unwrap_or_else(|e| panic!("contract test body failed: {}", e));
            });

            fn env_hook(
                $env: &std::rc::Rc<$crate::TestEnvironment>,
                $handler: &mut $crate::native_exec::transaction_handler::TransactionHandler,
            ) -> () $env_hook

            env_hook(&mut env, &mut handler);

            let res = handler.root_invoke();
            if res.output.status_code() != ::groundhog_xdr::transaction::InvocationFrameStatusCode::kInvocationFrameStatusCodeReturn as i32 {
                panic!("root invocation panicked with code {:?}", res.output);
            }

            let commit_res = handler.try_commit_transaction();
            assert_eq!(commit_res, groundhog_xdr::exec_result::ExecResult::ExecResultOK);

            fn post_hook(
                $handler2: &mut $crate::native_exec::transaction_handler::TransactionHandler,
            ) -> () $post_hook

            post_hook(&mut handler);
        }
    };
}

pub fn grant_storage_tokens(
    env: &TestEnvironmentHandle,
    handler: &native_exec::transaction_handler::TransactionHandler,
) {
    env.setup_grant_storage_tokens(
        &handler
            .get_signed_transaction()
            .tx
            .invocation
            .invoked_address,
        1_000_000_000,
    );
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
    let env = TestEnvironment::new(None);
    TestEnvironment::enable_syscall_logging();
    let contract_hash = env.setup_create_contract(&contract);

    let mut handler = env.prepare_test(move || -> () {
        let _ = test
            .run(&contract_hash)
            .unwrap_or_else(|e| panic!("run method failed: {}", e));
    });

    let res = handler.root_invoke();

    if res.output.status_code()
        != InvocationFrameStatusCode::kInvocationFrameStatusCodeReturn as i32
    {
        panic!("Root invocation panicked with code {:?}", res.output);
    }

    let gas_consumed = res.gas_consumed;

    // std::println!("gas consumed: {}", gas_consumed);

    match handler.try_commit_transaction() {
        ExecResult::ExecResultOK => {}
        // Note: these results are also deterministic:
        //     ExecResultTxDeterministicError
        //     ExecResultTxRootFrameReverted
        r => panic!("Transaction failed: {:?}", r),
    }

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

pub fn deploy_address(
    deployer_address: &ContractAddress,
    nonce: &Hash256,
    contract_hash: &Hash256,
) -> ContractAddress {
    use groundhog_xdr::contract::ContractDeploymentHashPreimage;
    use sha2::{Digest, Sha256};

    let preimage = ContractDeploymentHashPreimage {
        deployer_address: *deployer_address,
        nonce: *nonce,
        contract_hash: *contract_hash,
    };
    let preimage: Vec<u8> = xdrs::txs::encode(&preimage).unwrap();

    let mut hasher = Sha256::new();
    hasher.update(preimage.as_slice());

    let mut addr = [1u8; 32];
    hasher.finalize_into((&mut addr).into());

    addr.into()
}
