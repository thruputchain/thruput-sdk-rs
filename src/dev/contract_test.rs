use crate::call::{
    AbortBuf, CallResult, ContractError, ContractResult, ReturnBuf,
};
use crate::exn::{Exn, FaultString};
use crate::types::transaction::InvocationFrameOutput::*;
use crate::*;
use core::convert::Infallible;
use groundhog_native_exec as native_exec;
use groundhog_xdr::transaction::InvocationFrameStatusCode;
use native_exec::transaction_handler::TransactionHandler;
use std::panic::UnwindSafe;
use std::vec::Vec;

/// Execute the provided thunk in a transaction in the test environment.
///
pub fn contract_exec<F>(
    env: TestEnvironmentHandle,
    f: F,
) -> CallResult<(), Infallible>
where
    F: Fn() + UnwindSafe + Clone + 'static,
{
    contract_test(env, f)
}

/// Execute the provided code in a transaction in the test environment,
/// controlling the return type and exception type.
///
/// # Example:
/// ```
/// let RET = ();
/// let EXN = Infallible;
///
/// // Annotate the result type so that `contract_test` knows how to decode the return
/// // and exception types:
/// let call_result: CallResult<RET, EXN> = contract_test(env, || { /* whatever */ });
/// ```
///
pub fn contract_test<F, RET, EXN>(
    env: TestEnvironmentHandle,
    f: F,
) -> CallResult<RET, EXN>
where
    F: Fn() + UnwindSafe + Clone + 'static,
    RET: ReadXDROwned,
    EXN: Exn,
{
    crate::call::into_call_result(contract_test_raw(env, f))
}

/// Execute the provided thunk in a transaction in the test environment,
/// returning the raw `ContractResult`.
///
pub fn contract_test_raw<F>(env: TestEnvironmentHandle, f: F) -> ContractResult
where
    F: Fn() + UnwindSafe + Clone + 'static,
{
    let mut handler = contract_test_handler(env, f);

    contract_handler_exec_raw(&mut handler)
}

/// Make a transaction handler that will run the provided thunk.
///
pub fn contract_test_handler<F>(
    env: TestEnvironmentHandle,
    f: F,
) -> TransactionHandler
where
    F: Fn() + UnwindSafe + Clone + 'static,
{
    let test =
        move || with_abort_handler!(test_abort_handler::<Infallible>, f());

    env.prepare_test(test)
}

/// Invoke the handler's root frame and commit the transaction, decoding the
/// `ContractResult` into a `CallResult`.
///
pub fn contract_handler_exec(
    handler: &mut TransactionHandler,
) -> CallResult<(), Infallible> {
    crate::call::into_call_result(contract_handler_exec_raw(handler))
}

/// Invoke the handler's root frame and commit the transaction.
///
pub fn contract_handler_exec_raw(
    handler: &mut TransactionHandler,
) -> ContractResult {
    let res = handler.root_invoke();

    let return_buf = match res.output {
        kInvocationFrameStatusCodeReturn { return_buffer } => {
            // TODO: Unify these types and skip the copy
            ReturnBuf::from_slice(return_buffer.as_slice()).unwrap()
        }
        kInvocationFrameStatusCodeAbort { abort_buffer } => {
            // TODO: Unify these types and skip the copy
            let abort_buf =
                AbortBuf::from_slice(abort_buffer.as_slice()).unwrap();
            return Err(ContractError::Abort(abort_buf));
        }
        kInvocationFrameStatusCodeFault => {
            return Err(ContractError::Fault(FaultString::empty()));
        }
    };

    match handler.try_commit_transaction() {
        ExecResult::ExecResultOK => {}
        r => panic!("Transaction failed: {:?}", r),
    }

    Ok(return_buf)
}

/// Run some code in the test environment.
///
#[macro_export]
macro_rules! contract_test {
    ($name:ident $test_body:tt) => {
        #[test]
        fn $name() {
            let mut env = $crate::TestEnvironment::no_cost();

            $crate::contract_exec(env, || $test_body)
                .unwrap_or_else(|e| panic!("Contract test failed: {e}"));
        }
    };
}

/// Given a contact implementation and interface, create test
/// functions that take a client object as an argument.
///
#[macro_export]
macro_rules! client_test {
    {   $impl_ty:ty =>
        $(fn $test_name:ident($client:ident: &mut $client_ty:ty) $body:tt)* } =>
    {
        static CLIENT_TEST_NONCE: ::sdk::SHA256Hash = ::sdk::sha256_static!("CLIENT_TEST_NONCE");

        $(

        #[test]
        fn $test_name() {
            let mut env = ::sdk::TestEnvironment::no_cost();
            let test_contract = <$impl_ty as sdk::TestRegister>::register(&mut env);
            let contract_hash = env.setup_create_contract(&test_contract);

            ::sdk::contract_exec(env, move || {
                // Deploy the contract under test
                let addr = ::sdk::deploy_contract(&CLIENT_TEST_NONCE, &contract_hash);

                // Create a client to interact with the contract
                let mut client = <$client_ty>::new(&addr);

                (|$client: &mut $client_ty| $body)(&mut client);
            })
            .unwrap_or_else(|e|
                panic!("Client test failed: {}", e));
        }

        )*
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
        test.run(&contract_hash)
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
