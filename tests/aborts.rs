use sdk::call::ContractError;
use sdk::call::RawCallError;
use sdk::*;

#[test]
fn test_panic() {
    let env = TestEnvironment::no_cost();

    let result: Result<(), RawCallError> = contract_test(env, || {
        panic!("Just panic");
    });

    match &result {
        Err(RawCallError::Fault(_)) => {
            println!("OK: faulted as expected")
        }
        r => {
            panic!("Unexpected invocation result: {:?}", r)
        }
    }
}

#[test]
fn test_exception_raw() {
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

#[test]
fn test_unparseable_return() {
    let env = TestEnvironment::no_cost();

    // The contract "returns successfully" but with garbage bytes that
    // can't be XDR-decoded as the expected return type.
    let result: Result<u64, RawCallError> = contract_test(env, || {
        sdk::contract_return_buf(&[0xFF]);
    });

    match &result {
        Err(RawCallError::UnparseableReturn(_)) => {
            println!("OK: got UnparseableReturn as expected")
        }
        r => {
            panic!("Expected UnparseableReturn, got: {:?}", r)
        }
    }
}

#[test]
fn test_unparseable_abort() {
    let env = TestEnvironment::no_cost();

    // The contract aborts with garbage bytes that can't be parsed
    // as a RawException (needs at least a u64 selector).
    let result: Result<(), RawCallError> = contract_test(env, || {
        contract_abort(&[0xFF]);
    });

    match &result {
        Err(RawCallError::UnparseableAbort(_)) => {
            println!("OK: got UnparseableAbort as expected")
        }
        r => {
            panic!("Expected UnparseableAbort, got: {:?}", r)
        }
    }
}
