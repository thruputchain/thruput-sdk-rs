//! A mock method's panic must classify the same way regardless of which
//! thread runs it.
//!
//! `invoke_wrapper`'s catch site asks `is_panic_hook_active()` to tell a
//! contract panic (deterministic failure of one invocation) from a harness
//! bug (unrecoverable). Execution is coroutinized: `MockRuntime::
//! invoke_internal` runs on whichever thread the coroutine resumed on, never
//! reliably the one that entered the FFI call and activated the hook. So a
//! thread-affine answer to "is a frame active" misclassifies every panic that
//! lands on a resumed thread.
//!
//! Separate integration binary: these cases drive the process-global panic
//! hook, and the existing `invoke_error` binary serializes its cases for the
//! same reason.

use groundhog_native_exec::invoke_error::*;
use hogexec_internal::ffi_enums::InvokeErrorCode;

#[test]
fn contract_panic_classifies_the_same_on_a_resumed_thread() {
    activate_panic_hook();

    // Baseline: on the activating thread, an ordinary contract panic is a
    // deterministic failure of that invocation.
    let on_entering_thread =
        invoke_wrapper(|| panic!("contract panic"))().panic;

    // The same panic, in the same active frame, on a different thread --
    // which is what a resumed coroutine gives you.
    let on_resumed_thread = std::thread::spawn(|| {
        invoke_wrapper(|| panic!("contract panic"))().panic
    })
    .join()
    .expect("harness thread panicked");

    deactivate_panic_hook();

    assert_eq!(
        on_entering_thread,
        InvokeErrorCode::Deterministic.0,
        "baseline: the activating thread should classify a contract panic as \
         deterministic"
    );
    assert_eq!(
        on_resumed_thread, on_entering_thread,
        "a contract panic classified as {} on the entering thread became {} \
         on a resumed thread: the frame is active either way, so the answer \
         must not depend on which thread runs it",
        on_entering_thread, on_resumed_thread
    );
}
