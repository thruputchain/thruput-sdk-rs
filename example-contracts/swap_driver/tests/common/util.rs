// General debugging utilities

use sdk::*;
use std::panic::UnwindSafe;

pub type Result<T, E = TransactResult> = core::result::Result<T, E>;

#[track_caller]
pub fn transact_assert_success<F, T>(f: F) -> T
where
    F: Fn() -> Result<T>,
{
    // (Protect track_caller by avoiding a closure)
    match f() {
        Ok(v) => v,
        Err(r) => {
            r.assert_success();
            unreachable!();
        }
    }
}

/// Run `f` as a single auto-committed transaction at `addr`. On success the
/// closure's return value is decoded from the frame's return buffer; on
/// failure the full `TransactResult` is returned for inspection.
pub fn exec_tx<F, T>(
    env: &TestEnvironmentHandle,
    addr: &ContractAddress,
    f: F,
) -> Result<T, TransactResult>
where
    F: FnOnce() -> T + UnwindSafe,
    T: WriteXDR + ReadXDROwned,
{
    let r = env.transact(|tx| tx.call(addr).run(f));
    if r.is_success() {
        Ok(r.assert_return_as())
    } else {
        Err(r)
    }
}
