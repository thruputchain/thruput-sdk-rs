//! A call the runtime refuses before running the callee must not look like a
//! callee that ran and failed.
//!
//! Reentrance and the cross-invoke depth bound are reported by `Invoke` as
//! syscall errors, distinct from `kSyscallErrorCodeInvalidSyscallInput`. They
//! used to be stringified into the `Fault` variant's message, which made them
//! indistinguishable from a panicking callee — completely so on rv32, where
//! that message compiles out, and even on the host, where `FaultString`
//! compares equal to every other `FaultString`.

use sdk::call::{RawCallError, try_call_raw_with};
use sdk::exn::FaultString;
use sdk::invoke_config::InvokeFlagSet;
use sdk::*;

/// Call `target` from inside a frame already running at `caller`.
fn call_from_frame_at(
    caller: &str,
    target: &ContractAddress,
) -> RawCallError<()> {
    let env = TestEnvironment::no_cost();
    let target = target.clone();
    let mut captured = None;

    let result = env.transact(|tx| {
        // `AssertUnwindSafe`: the closure writes the call outcome out through
        // `&mut`, and the frame is discarded if it unwinds, so there is no
        // torn state for a later observer to see.
        tx.call(&ContractAddress::named(caller)).run(
            std::panic::AssertUnwindSafe(|| {
                let outcome: Result<(), RawCallError<()>> = try_call_raw_with(
                    &target,
                    [0u8; 8].into(),
                    u64::MAX,
                    &(),
                    InvokeFlagSet::default(),
                    |_| (),
                );
                captured =
                    Some(outcome.expect_err("a self-call must not succeed"));
            }),
        )
    });

    // The frame itself completed; the call inside it is what failed.
    result.assert_success();
    captured.expect("the call outcome must have been recorded")
}

/// Calling an address that already has a frame open is refused as reentrant,
/// and says so — it is not reported as the callee having faulted.
#[test]
fn self_call_reports_reentrance_not_fault() {
    let addr = ContractAddress::named("call_rejection_self");
    let err = call_from_frame_at("call_rejection_self", &addr);

    assert_eq!(
        err,
        RawCallError::Reentrance,
        "a reentrant call must be distinguishable from a faulting callee"
    );
    assert!(err.is_rejected_before_call());
}

/// The distinction has to survive as a *value*, not just as a rendered
/// message. `FaultString` compares equal to every other `FaultString` on
/// every target, and on rv32 it holds nothing at all, so a rejection sharing
/// the `Fault` variant compared equal to a genuine callee fault — no
/// assertion on the payload could have separated them. Keeping them in
/// distinct variants is what makes them distinguishable.
#[test]
fn reentrance_is_not_equal_to_fault() {
    let addr = ContractAddress::named("call_rejection_ne");
    let err = call_from_frame_at("call_rejection_ne", &addr);

    assert_ne!(err, RawCallError::Fault(FaultString::empty()));

    // The premise: any two `Fault`s are equal whatever their messages, so
    // the variant split is doing the work, not the payload.
    assert_eq!(
        RawCallError::<()>::Fault(FaultString::empty()),
        RawCallError::<()>::Fault(FaultString::from_display("boom")),
    );
}
