//! A faulting frame's panic message, where the harness can honestly report
//! one.
//!
//! The message is recorded at `MockInvocationFrame::execute`'s catch site,
//! which is the one place the payload and the frame it belongs to are both in
//! hand on a single thread. That bounds what is knowable, and the tests below
//! pin both halves of the bound across the four ways frames nest:
//!
//! | outer | inner | inner's message |
//! |---|---|---|
//! | mock  | —      | recorded on the frame |
//! | mock  | mock   | recorded on the *child*, never the parent |
//! | mock  | invoke | not recorded — `invoke_wrapper` has no frame |
//! | invoke| invoke | not recorded, and never leaks to an outer frame |
//!
//! "Not recorded" is the deliberate half: a caller must not be handed some
//! other frame's message, so absence reads as `"<fault>"` rather than as
//! whatever was lying around.

use sdk::call::{RawCallError, try_call_raw_with};
use sdk::contract::ContractEntrypointID;

use sdk::invoke_config::InvokeFlagSet;
use sdk::*;
use std::panic::AssertUnwindSafe;
use std::sync::{Arc, Mutex};

const ENTRY: [u8; 8] = [0; 8];

/// Deploy a mock contract at `addr` whose only entrypoint runs `body`.
fn deploy_mock<F>(env: &TestEnvironmentHandle, addr: &ContractAddress, body: F)
where
    F: Fn() + std::panic::UnwindSafe + Clone + Send + Sync + 'static,
{
    let mut c = env.add_contract();
    c.register_method(ContractEntrypointID::from(ENTRY), body);
    let _ = env.setup_create_contract(&c.as_contract());
    env.setup_create_deployment(addr, &c.as_contract());
}

/// Call `target` from inside a frame running at `caller`, returning both the
/// call's error and the calling frame's own recorded message.
fn call_from_frame(
    env: &TestEnvironmentHandle,
    caller: &ContractAddress,
    target: &ContractAddress,
) -> (Result<(), RawCallError<()>>, Option<String>) {
    let target = *target;
    let mut captured = None;

    let result = env.transact(|tx| {
        tx.call(caller).run(AssertUnwindSafe(|| {
            captured = Some(try_call_raw_with(
                &target,
                ContractEntrypointID::from(ENTRY),
                u64::MAX,
                &(),
                InvokeFlagSet::default(),
                |_| (),
            ));
        }))
    });

    (
        captured.expect("the call outcome must have been recorded"),
        result.fault_message().map(str::to_owned),
    )
}

/// The deliverable: a root frame that panics hands its message to the caller
/// instead of the bare `"<fault>"` placeholder.
#[test]
fn root_frame_panic_message_reaches_the_caller() {
    let env = TestEnvironment::no_cost();
    let result = env.transact(|tx| {
        tx.call(&ContractAddress::named("fault_msg_root"))
            .run(|| -> () { panic!("the specific thing that went wrong") })
    });

    assert_eq!(
        result.fault_message(),
        Some("the specific thing that went wrong")
    );

    let res: Result<(), RawCallError> = into_root_call_result(&result);
    match res {
        Err(RawCallError::Fault(f)) => {
            assert_eq!(f.as_str(), "the specific thing that went wrong")
        }
        other => panic!("expected Fault, got {other:?}"),
    }
}

/// `panic!` with a format produces a `String` payload rather than a
/// `&'static str`; both shapes have to survive, or the common case of a
/// message with a value interpolated into it silently degrades.
#[test]
fn formatted_panic_message_survives() {
    let env = TestEnvironment::no_cost();
    let result = env.transact(|tx| {
        tx.call(&ContractAddress::named("fault_msg_fmt"))
            .run(|| -> () { panic!("balance {} below minimum {}", 7, 100) })
    });

    assert_eq!(result.fault_message(), Some("balance 7 below minimum 100"));
}

/// A frame that does not fault reports no message, so a stale one can never
/// be read back as though it belonged to a later, successful run.
#[test]
fn a_frame_that_did_not_fault_has_no_message() {
    let env = TestEnvironment::no_cost();
    let result = env.transact(|tx| {
        tx.call(&ContractAddress::named("fault_msg_ok")).run(|| {})
    });

    result.assert_success();
    assert_eq!(result.fault_message(), None);
}

/// mock → mock. A child frame catches its own panic and closes itself, so the
/// panic never reaches the parent's catch site. The message belongs to the
/// child's result; the parent, which completed fine, has none.
#[test]
fn child_mock_frame_keeps_its_own_message() {
    let env = TestEnvironment::no_cost();
    let parent_addr = ContractAddress::named("fault_msg_parent");
    let child_addr = ContractAddress::named("fault_msg_child");

    // `AssertUnwindSafe` implements only the zero-argument `FnOnce`, so a
    // closure taking the frame handle has to be `UnwindSafe` on its own.
    let child_message: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let child_slot = Arc::clone(&child_message);
    let parent = env.transact(|tx| {
        tx.call(&parent_addr)
            .run_continuable(move |frame: &mut FrameHandle| {
                let child = frame
                    .child_call(&child_addr)
                    .run(|| -> () { panic!("child body exploded") });
                *child_slot.lock().unwrap() =
                    child.fault_message().map(str::to_owned);
                FrameAction::Close
            })
            .finish()
    });

    assert_eq!(
        child_message.lock().unwrap().as_deref(),
        Some("child body exploded")
    );
    assert_eq!(
        parent.fault_message(),
        None,
        "a child's panic must not be attributed to the parent frame"
    );
}

/// mock → invoke. The callee ran in a frame the runtime owns; its panic was
/// caught by `invoke_wrapper`, which has no frame to record against, so the
/// caller gets `Fault` with no message rather than someone else's.
///
/// The panic text still reaches the developer — the panic hook prints it to
/// stderr — it just is not carried as a value.
#[test]
fn callee_fault_carries_no_message_and_leaks_none() {
    let env = TestEnvironment::no_cost();
    let caller_addr = ContractAddress::named("fault_msg_caller");
    let callee_addr = ContractAddress::named("fault_msg_callee");
    deploy_mock(&env, &callee_addr, || panic!("callee exploded"));

    let (outcome, caller_message) =
        call_from_frame(&env, &caller_addr, &callee_addr);

    match outcome.expect_err("the callee panics, so the call fails") {
        RawCallError::Fault(f) => assert_eq!(
            f.as_str(),
            "<fault>",
            "a callee's message is not available on this path; reporting \
             anything else would mean reporting the wrong frame's"
        ),
        other => panic!("expected Fault, got {other:?}"),
    }
    assert_eq!(
        caller_message, None,
        "the calling frame did not itself panic, so it has no message"
    );
}

/// invoke → invoke. The inner callee's panic must not surface on the outer
/// callee's frame, nor climb out to the root — each `invoke_wrapper` catch is
/// independent and records nothing.
#[test]
fn nested_invoke_fault_does_not_leak_outward() {
    let env = TestEnvironment::no_cost();
    let root_addr = ContractAddress::named("fault_msg_nested_root");
    let outer_addr = ContractAddress::named("fault_msg_nested_outer");
    let inner_addr = ContractAddress::named("fault_msg_nested_inner");

    deploy_mock(&env, &inner_addr, || panic!("inner exploded"));

    // The middle contract calls the panicking one and reports what it saw.
    let seen: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let seen_for_body = Arc::clone(&seen);
    deploy_mock(&env, &outer_addr, move || {
        let outcome: Result<(), RawCallError<()>> = try_call_raw_with(
            &inner_addr,
            ContractEntrypointID::from(ENTRY),
            u64::MAX,
            &(),
            InvokeFlagSet::default(),
            |_| (),
        );
        if let Err(RawCallError::Fault(f)) = outcome {
            *seen_for_body.lock().unwrap() = Some(f.as_str().to_owned());
        }
    });

    let (outcome, root_message) =
        call_from_frame(&env, &root_addr, &outer_addr);

    assert_eq!(
        seen.lock().unwrap().as_deref(),
        Some("<fault>"),
        "the middle contract sees a message-less fault, not the inner text"
    );
    // The middle contract does not re-throw, so it returns normally and the
    // root's call never reaches a Fault at all.
    assert!(
        outcome.is_ok(),
        "the middle contract swallows the fault, so the outer call succeeds: \
         {outcome:?}"
    );
    assert_eq!(root_message, None);
}
