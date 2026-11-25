//! Runs a test body in a native transaction.
//!
//! The arithmetic does not need one: off a contract, `thruput-u256` divides
//! and takes square roots in software (`tests/host.rs` runs without). These
//! tests keep the transaction so they run as a contract's native tests do. A
//! panic in the body is carried out of the transaction and resumed here, so
//! `#[should_panic]` sees the body's own message.

use std::panic::{self, AssertUnwindSafe};
use thruput_native_exec::test_environment::TestEnvironment;
use thruput_native_exec::test_helpers::Named;
use thruput_xdr::types::ContractAddress;

pub fn native(body: impl FnOnce()) {
    let mut outcome = None;
    let env = TestEnvironment::no_cost();
    env.transact(|tx| {
        tx.call(&ContractAddress::named("test"))
            .run(AssertUnwindSafe(|| {
                outcome = Some(panic::catch_unwind(AssertUnwindSafe(body)));
            }))
    })
    .assert_success();
    if let Err(payload) = outcome.expect("the transaction ran the body") {
        panic::resume_unwind(payload);
    }
}
