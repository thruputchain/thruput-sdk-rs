//! Test sugar over the fluent mock-execution API.
//!
//! The canonical test pattern is:
//! ```rust,ignore
//! let env = TestEnvironment::no_cost();
//! let result = env.transact(|tx| tx.call(&addr).run(|| { /* contract code */ }));
//! result.assert_success();
//! ```
//! `contract_test!` and `client_test` are thin sugar over that pattern.
//! `test_wasm` is the harness for running real wasm blobs through the full
//! SignedTransaction path (the only place gas_consumed is observable).

use crate::call::RawCallError;
use crate::exn::FaultString;
use crate::exn::RawException;
use crate::types::transaction::InvocationFrameOutput::*;
use crate::*;
use groundhog_native_exec as native_exec;
use groundhog_xdr::auth_claim::{AuthClaim, AuthClaim_source_address_claim};
use groundhog_xdr::contract::ContractEntrypointID;
use groundhog_xdr::transaction::{
    Invocation, InvocationFrameStatusCode, SignedTransaction, Transaction,
    Uniquifier,
};
use native_exec::transaction_handler::TransactionHandler;
use std::panic::UnwindSafe;
use std::string::ToString;
use std::vec;
use std::vec::Vec;

/// Decode a `TransactResult`'s root-frame outcome into the same
/// `Result<T, RawCallError>` shape that contract clients produce.
/// Use this when a test asserts on typed exceptions/selectors rather
/// than raw buffers.
///
/// The success path requires a committed transaction (panics on commit
/// failure); Abort/Fault decode frame-only.
pub fn into_root_call_result<T>(
    result: &TransactResult,
) -> Result<T, RawCallError>
where
    T: ReadXDROwned + ReceiveAuthHandles,
{
    match result.frame().output() {
        Return { return_buffer } => {
            // A successful frame whose commit failed must not decode to Ok —
            // mirror the legacy wrappers, which panicked on commit failure.
            match result.exec_result() {
                &ExecResult::ExecResultOK => {}
                r => panic!("Transaction failed: {:?}", r),
            }
            // No returned auth handles: the root frame has no caller to hand
            // them back to.
            crate::call::contract_call_decode::<T>(
                return_buffer.as_slice(),
                &[],
            )
            .map_err(crate::call::widen_uninhabited_exn)
        }
        // Kept raw, not typed-decoded: callers assert on the selector and
        // payload of whatever came back.
        Abort { abort_buffer } => {
            match RawException::from_abort_buf(abort_buffer.as_slice()) {
                Ok(exn) => Err(RawCallError::Exception(exn)),
                Err(e) => Err(RawCallError::UnparseableAbort(
                    FaultString::from_display(e),
                )),
            }
        }
        Fault => Err(RawCallError::Fault(FaultString::empty())),
        other => {
            panic!("unsupported InvocationFrameOutput variant: {:?}", other)
        }
    }
}

/// Run some code in the test environment, as a single auto-committed
/// transaction at a deterministic per-test address.
///
#[macro_export]
macro_rules! contract_test {
    ($name:ident $test_body:tt) => {
        #[test]
        fn $name() {
            let env = $crate::TestEnvironment::no_cost();
            env.transact(|tx| {
                let addr = <$crate::ContractAddress as $crate::Named>::named(
                    stringify!($name),
                );
                tx.call(&addr).run(|| $test_body)
            })
            .assert_success();
        }
    };
}

pub trait TestRegisterFn: Fn(&TestEnvironmentHandle) -> MockContract {}

impl<T> TestRegisterFn for T where T: Fn(&TestEnvironmentHandle) -> MockContract
{}

pub fn client_test<R, F, C>(register: R, run_test: F)
where
    R: TestRegisterFn,
    F: FnOnce(&C) + UnwindSafe,
    C: crate::call::ContractClient,
{
    static CLIENT_TEST_NONCE: SHA256Hash =
        SHA256Hash::new(hash_to_array!("CLIENT_TEST_NONCE"));

    let env = TestEnvironment::no_cost();
    let test_contract = register(&env);
    let contract_hash = *test_contract.as_hash();

    env.transact(|tx| {
        let caller = <ContractAddress as Named>::named("client_test");
        tx.call(&caller).run(move || {
            // Deploy the contract under test and hand the caller a client.
            let addr = deploy_contract(&CLIENT_TEST_NONCE, &contract_hash);
            let client = C::new(&addr);
            run_test(&client);
        })
    })
    .assert_success();
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

/// Build a one-shot TransactionHandler that runs `test` as a mock contract
/// method through the full SignedTransaction path. Internal harness for
/// `test_wasm` — everything else uses `env.transact`.
fn wasm_test_handler<F>(
    env: &TestEnvironmentHandle,
    test: F,
) -> TransactionHandler
where
    F: Fn() + UnwindSafe + Clone + 'static,
{
    let mut test_contract = env.add_contract();
    let entrypoint = ContractEntrypointID::from([0; 8]);
    test_contract.register_method(entrypoint, test);
    let _ = env.setup_create_contract(&test_contract.as_contract());

    let invoke_addr: ContractAddress = [0xFF; 32].into();

    env.setup_create_deployment(&invoke_addr, &test_contract.as_contract());

    let default_auth_claim: AuthClaim = AuthClaim::kSourceAddressAuthData {
        source_address_claim: AuthClaim_source_address_claim {
            allowed_keys: 0,
            required_weight: 0,
        },
    };

    let stx = SignedTransaction {
        signatures: Vec::new().try_into().unwrap(),
        tx: Transaction {
            uniquifier: Uniquifier {
                block_expiry_height: 0.into(),
                nonce: 0,
            },
            auth_claim: default_auth_claim,
            invocation: Invocation {
                invoked_address: invoke_addr,
                entrypoint,
                gas_limit: u64::MAX,
                calldata: [].try_into().unwrap(),
            },
            contracts_to_deploy: Vec::new().try_into().unwrap(),
        },
    };

    TransactionHandler::new(env.clone(), &stx)
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

    let mut handler = wasm_test_handler(&env, move || -> () {
        test.run(&contract_hash)
            .unwrap_or_else(|e| panic!("run method failed: {}", e));
    });

    let res = handler.root_invoke();

    if res.output.status_code() != InvocationFrameStatusCode::Return {
        panic!("Root invocation panicked with code {:?}", res.output);
    }

    let gas_consumed = res.gas_consumed;

    // std::println!("gas consumed: {}", gas_consumed);

    match handler.try_commit_transaction() {
        native_exec::TryCommitResult::Ok => {}
        // Note: these results are also deterministic:
        //     TxDeterministicError
        //     TxRootFrameReverted
        r => panic!("Transaction failed: {:?}", r),
    }

    Stats { gas_consumed }
}

pub fn load_wasm_contract<P>(path: P) -> Contract
where
    P: AsRef<std::path::Path>,
{
    load_wasm_contract_instrumented(path, true)
}

pub fn load_wasm_contract_instrumented<P>(
    path: P,
    please_instrument: bool,
) -> Contract
where
    P: AsRef<std::path::Path>,
{
    use crate::contract::ContractBlob;

    let raw_wasm = std::fs::read(path.as_ref()).unwrap_or_else(|e| {
        panic!(
            "{}: couldn't load wasm: {}",
            path.as_ref().to_str().unwrap_or("?"),
            e
        )
    });

    let wasm = if please_instrument {
        // Mirrors hogd's `verify_contract_meter` config (hogd/rust/common/src/lib.rs
        // at the pinned test-ffi tag) so the mock accepts exactly what the chain does.
        let instrument_config = groundhog_wasm_meter::Config {
            max_imported_functions: 1,
            max_table_initial_size: 64,
            reject_floats: true,
            allowed_custom_sections: vec![
                "name".to_string(),
                "groundhogspecv0".to_string(),
                "groundhogspecrawv1".to_string(),
            ],
            ..groundhog_wasm_meter::Config::default()
        };

        groundhog_wasm_meter::instrument(&raw_wasm, &instrument_config)
            .unwrap_or_else(|e| {
                panic!(
                    "{}: wasm instrumentation failed: {}",
                    path.as_ref().to_str().unwrap_or("?"),
                    e
                )
            })
    } else {
        raw_wasm
    };

    let contract: ContractBlob = ContractBlob { data: BytesM(wasm) };

    Contract::kWasmi {
        wasmi_contract: contract,
    }
}

pub fn load_rv32_contract<P>(path: P) -> Contract
where
    P: AsRef<std::path::Path>,
{
    use crate::contract::ContractBlob;

    let raw_rv32 = std::fs::read(path.as_ref()).unwrap_or_else(|e| {
        panic!(
            "{}: couldn't load rv32 contract: {}",
            path.as_ref().to_str().unwrap_or("?"),
            e
        )
    });

    let contract: ContractBlob = ContractBlob {
        data: BytesM(raw_rv32),
    };

    Contract::kRv32 {
        rv32_contract: contract,
    }
}

pub fn deploy_address(
    deployer_address: &ContractAddress,
    nonce: &SHA256Hash,
    contract_hash: &SHA256Hash,
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

#[cfg(test)]
mod tests {
    use super::load_rv32_contract;
    use super::wasm_test_handler;
    use crate::TestEnvironment;
    use std::format;
    use std::fs;
    use std::string::String;

    #[test]
    fn panic_with_live_transaction_handler_survives_drop() {
        // Parity with native-exec's
        // `panic_with_open_transaction_is_not_masked_by_drop`, but for the real
        // `TransactionHandler` (full SignedTransaction) path: a test that
        // panics while a handler is live (invoked but not yet committed) must
        // let the original panic propagate, not have it masked by a second
        // panic from `Drop`'s FFI teardown.
        //
        // This drives the clean-abort path — the FFI abort/close succeed, so
        // the only way `Drop` could double-panic here is if it panicked
        // unconditionally (as it did before the teardown errors were routed
        // through `drop_failure`). We can't inject an FFI teardown *failure*
        // from a unit test, so this guards against reintroducing an
        // unconditional Drop panic rather than reproducing the conditional
        // double-failure that the fix ultimately hardens against.
        let caught =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let env = TestEnvironment::new(None);
                let mut handler = wasm_test_handler(&env, || {});
                // Transaction invoked but never committed or aborted: Drop must
                // abort it during unwind.
                let _ = handler.root_invoke();
                panic!("original failure message");
            }));

        let payload = caught.expect_err("the panic must propagate, not abort");
        let msg = payload
            .downcast_ref::<&'static str>()
            .copied()
            .or_else(|| payload.downcast_ref::<String>().map(String::as_str));
        assert_eq!(
            msg,
            Some("original failure message"),
            "the original panic message must survive Drop cleanup"
        );
    }

    #[test]
    fn load_rv32_contract_wraps_artifact_bytes() {
        let bytes = b"fake-rv32-artifact";
        let path = std::env::temp_dir().join(format!(
            "groundhog-sdk-load-rv32-contract-{}.rv32a",
            std::process::id()
        ));
        fs::write(&path, bytes).unwrap();

        let contract = load_rv32_contract(&path);

        fs::remove_file(&path).unwrap();
        match contract {
            groundhog_xdr::contract::Contract::kRv32 { rv32_contract } => {
                assert_eq!(rv32_contract.data.0, bytes);
            }
            other => panic!("expected rv32 contract, got {other:?}"),
        }
    }
}
