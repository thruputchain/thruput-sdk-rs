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

use sdk::*;
use sdk::call::CallError;

#[test]
fn test_panic() {
    let env = TestEnvironment::new();

    let result = contract_test_invoke(env, || {
        panic!("Just panic");
    });

    match result {
        Err(e @ CallError::Abort(exn)) => {
            if let Some(le) = exn.as_library_exn()
                && let Some(se) = le.as_syscall_exn()
                    && se.has_code(SyscallErrorCode::kSyscallErrorCodeDeterministicWasm)
            {
                println!("OK: failed as expected: {e}")
            } else {
                panic!("Unexpected exception: {exn}");
            }
        },
        r => {
            panic!("Unexpected invocation result: {:?}", r)
        }
    }
}

/*
#[test]
fn test_abort_bare() {
    let env = TestEnvironment::new();

    let result = contract_test_invoke(env, || {
        contract_abort(&[1u8, 2, 3]);
    });

    match result {
        Err(e @ InvokeError::AbortedWith(3)) => {
            println!("Aborted as expected: {e}")
        }
        r => {
            panic!("Unexpected invocation result: {:?}", r)
        }
    }
}
*/
