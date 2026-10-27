//! A call the runtime refuses before running the callee must not look like a
//! callee that ran and failed.
//!
//! Reentrance and the cross-invoke depth bound are reported by `Invoke` as
//! syscall errors, distinct from `kSyscallErrorCodeInvalidSyscallInput`. They
//! used to be stringified into the `Fault` variant's message, which made them
//! indistinguishable from a panicking callee.

use std::sync::{Arc, Mutex};
use thruput_sdk::InvokeFlagSet;
use thruput_sdk::PanicRecord;
use thruput_sdk::call::{RawCallError, try_call_raw_with};
use thruput_sdk::contract::ContractEntrypointID;
use thruput_sdk::exn::FaultInfo;
use thruput_sdk::*;

/// Call `target` from inside a frame already running at `caller`.
fn call_from_frame_at(
    caller: &str,
    target: &ContractAddress,
) -> RawCallError<()> {
    let env = TestEnvironment::no_cost();
    let target = *target;
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
/// message. `FaultInfo` compares equal to every other `FaultInfo` on every
/// target, and on rv32 -- or any build without `dev` -- it holds nothing at
/// all, so a rejection sharing the `Fault` variant compared equal to a
/// genuine callee fault: no assertion on the payload could have separated
/// them. Keeping them in distinct variants is what makes them
/// distinguishable.
#[test]
fn reentrance_is_not_equal_to_fault() {
    let addr = ContractAddress::named("call_rejection_ne");
    let err = call_from_frame_at("call_rejection_ne", &addr);

    assert_ne!(err, RawCallError::Fault(FaultInfo::empty()));

    // The premise: any two `Fault`s are equal whatever their messages, so
    // the variant split is doing the work, not the payload.
    assert_eq!(
        RawCallError::<()>::Fault(FaultInfo::empty()),
        RawCallError::<()>::Fault(FaultInfo::from(&PanicRecord {
            message: Some("boom".into()),
            location: None,
        })),
    );
}

/// Run root -> middle -> leaf, returning what refused the middle contract's
/// call to the leaf, or `None` if it went through. That call opens the third
/// frame of the transaction, so a `max_cross_invoke_depth` below three
/// refuses it.
fn third_frame_refusal(
    env: &TestEnvironmentHandle,
) -> Option<RawCallError<()>> {
    let entry = ContractEntrypointID::from([0u8; 8]);
    let leaf = ContractAddress::named("call_rejection_depth_leaf");
    let middle = ContractAddress::named("call_rejection_depth_middle");

    // Deployed, so the depth bound is the only thing that can refuse the
    // call: under a cap that admits it, the leaf runs.
    env.register_mock(|c| c.register_method(entry, || {}))
        .deploy_contract_to(&leaf);

    let refusal: Arc<Mutex<Option<RawCallError<()>>>> =
        Arc::new(Mutex::new(None));
    let refusal_for_body = Arc::clone(&refusal);
    env.register_mock(|c| {
        c.register_method(entry, move || {
            *refusal_for_body.lock().unwrap() =
                try_call_raw_with::<_, (), ()>(
                    &leaf,
                    entry,
                    u64::MAX,
                    &(),
                    InvokeFlagSet::default(),
                    |_| (),
                )
                .err();
        })
    })
    .deploy_contract_to(&middle);

    env.transact(|tx| {
        tx.call(&ContractAddress::named("call_rejection_depth_root"))
            .run(move || {
                // A second frame is within every cap tested here.
                try_call_raw_with::<_, (), ()>(
                    &middle,
                    entry,
                    u64::MAX,
                    &(),
                    InvokeFlagSet::default(),
                    |_| (),
                )
                .expect("the second frame must be admitted");
            })
    })
    .assert_success();

    refusal.lock().unwrap().take()
}

/// The cross-invoke depth bound is a chain setting, so a test can pin it
/// rather than discover it: `TestEnvironment::builder` states a
/// `max_cross_invoke_depth` of its own, and `Invoke` refuses the hop that
/// would exceed exactly that.
#[test]
fn depth_exceeded_is_reported_at_the_configured_cap() {
    let env = TestEnvironment::builder()
        .no_cost()
        .with_max_cross_invoke_depth(2)
        .build();

    let err = third_frame_refusal(&env)
        .expect("a third frame must exceed a cap of two");

    assert_eq!(err, RawCallError::DepthExceeded);
    assert!(err.is_rejected_before_call());
}

/// The other half of the claim: the cap is what refused it, not the shape of
/// the call. The same three frames are fine under the stock configuration.
#[test]
fn the_stock_cap_admits_the_same_third_frame() {
    let env = TestEnvironment::no_cost();

    assert_eq!(third_frame_refusal(&env), None);
}
