use groundhog_native_exec::invoke_error::*;
use hogexec_internal::ffi_enums::InvokeErrorCode;
use std::panic;
use std::panic::panic_any;

// There's a race condition on the panic handler,
// which changes how tests behave. This is an integration test
// (separate binary) so it won't conflict with other test binaries.
// We run all sub-tests sequentially within a single #[test] to
// avoid intra-binary parallelism on the global panic hook.
#[test]
fn test_panic_runner() {
    test_error_wrap();
    test_panic();
    test_panic_ok();
    test_sdk_panic();
    test_option_panic();
    test_pass_invoke_error_through_panic_hook();
}

fn test_error_wrap() {
    let _ = panic::take_hook();
    let f: fn() -> () = || {
        return;
    };

    let boxed = invoke_wrapper(f);

    assert_eq!((*boxed)().panic, InvokeErrorCode::kInvokeNone.0);
}

fn test_panic() {
    let _ = panic::take_hook();
    let f: fn() -> () = || panic!("unknown");

    let boxed = invoke_wrapper(f);

    assert_eq!((*boxed)().panic, InvokeErrorCode::kInvokeUnrecoverable.0);
}

fn test_panic_ok() {
    let _ = panic::take_hook();
    let f: fn() -> () = || panic_any(validation_error());

    let boxed = invoke_wrapper(f);

    assert_eq!(
        (*boxed)().panic,
        InvokeErrorCode::kInvokeValidationFailure.0
    );
}

fn test_sdk_panic() {
    set_override_panic_hook();
    let f: fn() -> () = || {
        panic!("something");
    };

    let boxed = invoke_wrapper(f);

    assert_eq!((*boxed)().panic, InvokeErrorCode::kInvokeDeterministic.0);
}

fn test_option_panic() {
    set_override_panic_hook();
    let f: fn() -> () = || {
        let x: Option<i32> = None;
        x.unwrap();
    };

    let boxed = invoke_wrapper(f);

    assert_eq!((*boxed)().panic, InvokeErrorCode::kInvokeDeterministic.0);
}

fn test_pass_invoke_error_through_panic_hook() {
    set_override_panic_hook();
    let f: fn() -> () = || panic_any(validation_error());

    let boxed = invoke_wrapper(f);

    assert_eq!(
        (*boxed)().panic,
        InvokeErrorCode::kInvokeValidationFailure.0
    );
}
