use crate::*;
use crate::call::{ContractResult, ContractError, ReturnBuf, AbortBuf, CallResult};
use crate::exn::{Exn, FaultString};
use crate::types::transaction::InvocationFrameOutput::*;
use groundhog_xdr::transaction::InvocationFrameStatusCode;
use std::panic::UnwindSafe;
use std::vec::Vec;
use groundhog_native_exec as native_exec;
use core::convert::Infallible;

pub fn contract_exec<F>(env: TestEnvironmentHandle, f: F) -> CallResult<(), Infallible>
where
    F: Fn() -> () + UnwindSafe + Clone + 'static,
{
    contract_test(env, f)
}

pub fn contract_test<F, RET, EXN>(env: TestEnvironmentHandle, f: F) -> CallResult<RET, EXN>
where
    F: Fn() -> () + UnwindSafe + Clone + 'static,
    RET: ReadXDROwned,
    EXN: Exn,
{
    crate::call::into_call_result(contract_test_raw(env, f))
}

pub fn contract_test_raw<F>(env: TestEnvironmentHandle, f: F) -> ContractResult
where
    F: Fn() -> () + UnwindSafe + Clone + 'static,
{
    let mut handler = env.prepare_test(f);
    // XXX env.prepare_test(|| sdk::with_abort_handler(&sdk::test_abort_handler::<core::convert::Infallible>, f))

    let res = handler.root_invoke();

    let return_buf = match res.output {
        kInvocationFrameStatusCodeReturn { return_buffer }=> {
            // TODO: Unify these types and skip the copy
            ReturnBuf::from_slice(return_buffer.as_slice()).unwrap()
        }
        kInvocationFrameStatusCodeAbort { abort_buffer } => {
            // TODO: Unify these types and skip the copy
            let abort_buf = AbortBuf::from_slice(abort_buffer.as_slice()).unwrap();
            return Err(ContractError::Abort(abort_buf));
        }
        kInvocationFrameStatusCodeFault => {
            return Err(ContractError::Fault(FaultString::empty()));
        }
    };

    match handler.try_commit_transaction() {
        ExecResult::ExecResultOK => {}
        r => panic!("Contract test: Unexpected transaction result: {:?}", r),
    }

    Ok(return_buf)
}

#[macro_export]
macro_rules! contract_test {
    ($name:ident $test_body:tt) => {
        #[test]
        fn $name() {
            let mut env = $crate::TestEnvironment::no_cost();

            $crate::contract_exec(env, || {
                $test_body

                // contract_return_empty(); // unnecessary?
            })
            .unwrap_or_else(|e|
                panic!("Contract test failed: {e}"));
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
                    $crate::deploy::deploy_contract(&CONTRACT_TEST_NONCE, &contract_hash)
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
