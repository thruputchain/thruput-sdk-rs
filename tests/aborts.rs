// XXX
// When run in parallel with the other test, this sometimes fails with:
//   E[syscall_handler.cc:217] handle_root_host_fn_status: received kHostUnrecoverable
/*
test test_abort_bare ... ok
test test_panic ... FAILED

failures:

---- test_panic stdout ----

thread 'test_panic' (411211) panicked at tests/aborts.rs:8:9:
Just panic
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

thread 'test_panic' (411211) panicked at native_exec/src/host_fn_error.rs:29:13:
Box<dyn Any>

thread 'test_panic' (411211) panicked at /home/dbg/gh/sdk/src/dev/contract_test.rs:84:9:
Root invocation panicked with code kInvokeUnrecoverable


failures:
    test_panic

test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.58s

error: test failed, to rerun pass `--test aborts`
*/

use sdk::call::ContractError;
use sdk::call::{CallError, CallResult};
use sdk::*;

#[test]
fn test_panic() {
    let env = TestEnvironment::no_cost();

    type RET = ();
    type EXN = core::convert::Infallible;

    let result: CallResult<RET, EXN> = contract_test(env, || {
        panic!("Just panic");
    });

    match &result {
        Err(CallError::Fault(_)) => {
            println!("OK: faulted as expected")
        }
        r => {
            panic!("Unexpected invocation result: {:?}", r)
        }
    }
}

#[test]
fn test_abort_raw() {
    let env = TestEnvironment::no_cost();

    let result = contract_test_raw(env, || {
        contract_abort(&[1u8, 2, 3]);
    });

    match result {
        Err(ContractError::Abort(buf)) if buf.as_slice() == &[1u8, 2, 3] => {
            println!("Aborted as expected!")
        }
        Err(ContractError::Fault(_)) => {
            panic!("Unexpectedly faulted");
        }
        r => {
            panic!("Unexpected invocation result: {:?}", r);
        }
    }
}
