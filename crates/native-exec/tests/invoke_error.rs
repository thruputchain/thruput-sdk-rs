use groundhog_native_exec::invoke_error::*;
use hogexec_internal::ffi_enums::InvokeErrorCode;
use std::panic;
use std::panic::panic_any;

/// These tests exercise panic conversion, not frame accounting; wrap
/// with a throwaway contract-index/entrypoint label.
fn wrap(f: fn()) -> Box<dyn Fn() -> groundhog_sys::ForeignInvokeResult> {
    invoke_wrapper(
        groundhog_xdr::contract::Contract::kMockTest {
            mock_contract_id: 0,
        },
        groundhog_xdr::contract::ContractEntrypointID::from_u64(0),
        f,
    )
}

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
    let f: fn() -> () = || {};

    let boxed = wrap(f);

    assert_eq!((*boxed)().panic, InvokeErrorCode::None.0);
}

fn test_panic() {
    let _ = panic::take_hook();
    let f: fn() -> () = || panic!("unknown");

    let boxed = wrap(f);

    assert_eq!((*boxed)().panic, InvokeErrorCode::Unrecoverable.0);
}

fn test_panic_ok() {
    let _ = panic::take_hook();
    let f: fn() -> () = || panic_any(validation_error());

    let boxed = wrap(f);

    assert_eq!((*boxed)().panic, InvokeErrorCode::ValidationFailure.0);
}

fn test_sdk_panic() {
    activate_panic_hook();
    let f: fn() -> () = || {
        panic!("something");
    };

    let boxed = wrap(f);

    assert_eq!((*boxed)().panic, InvokeErrorCode::Deterministic.0);
}

fn test_option_panic() {
    activate_panic_hook();
    // The `unwrap` on a known `None` is the point: it is the stand-in for an
    // ordinary contract-side panic, which the hook must convert.
    #[allow(clippy::unnecessary_literal_unwrap)]
    let f: fn() -> () = || {
        let x: Option<i32> = None;
        x.unwrap();
    };

    let boxed = wrap(f);

    assert_eq!((*boxed)().panic, InvokeErrorCode::Deterministic.0);
}

fn test_pass_invoke_error_through_panic_hook() {
    activate_panic_hook();
    let f: fn() -> () = || panic_any(validation_error());

    let boxed = wrap(f);

    assert_eq!((*boxed)().panic, InvokeErrorCode::ValidationFailure.0);
}
