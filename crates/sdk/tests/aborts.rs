use sdk::call::RawCallError;
use sdk::*;

fn run_at_test_addr(
    f: impl FnOnce() + std::panic::UnwindSafe,
) -> TransactResult {
    let env = TestEnvironment::no_cost();
    env.transact(|tx| {
        tx.call(&<ContractAddress as Named>::named("aborts")).run(f)
    })
}

#[test]
fn test_panic() {
    let result = run_at_test_addr(|| {
        panic!("Just panic");
    });
    result.assert_fault();
    let res: Result<(), RawCallError> = into_root_call_result(&result);
    assert!(
        matches!(res, Err(RawCallError::Fault(_))),
        "Unexpected invocation result: {:?}",
        res
    );
}

#[test]
fn test_exception_raw() {
    let result = run_at_test_addr(|| {
        contract_abort(&[1u8, 2, 3]);
    });
    result.assert_abort();
    assert_eq!(result.frame().abort_buffer(), &[1u8, 2, 3]);
}

#[test]
fn test_unparseable_return() {
    // The contract "returns successfully" but with garbage bytes that
    // can't be XDR-decoded as the expected return type.
    let result = run_at_test_addr(|| {
        sdk::contract_return_buf(&[0xFF]);
    });
    let res: Result<u64, RawCallError> = into_root_call_result(&result);
    assert!(
        matches!(res, Err(RawCallError::UnparseableReturn(_))),
        "Expected UnparseableReturn, got: {:?}",
        res
    );
}

#[test]
fn test_unparseable_abort() {
    // The contract aborts with garbage bytes that can't be parsed
    // as a RawException (needs at least a u64 selector).
    let result = run_at_test_addr(|| {
        contract_abort(&[0xFF]);
    });
    let res: Result<(), RawCallError> = into_root_call_result(&result);
    assert!(
        matches!(res, Err(RawCallError::UnparseableAbort(_))),
        "Expected UnparseableAbort, got: {:?}",
        res
    );
}
