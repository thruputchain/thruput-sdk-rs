//! Runtime exit (trap)

/// Abort the current invocation with a deterministic error.
///
/// Under `dev`, this panics with the mock executor's deterministic error,
/// which the native test environment reports as a trap.
#[cfg(feature = "dev")]
pub fn trap() -> ! {
    use std::panic::panic_any;
    use thruput_native_exec::invoke_error::deterministic_error;

    panic_any(deterministic_error())
}

/// Abort the current invocation with a deterministic error.
#[cfg(not(feature = "dev"))]
#[inline(always)]
pub fn trap() -> ! {
    panic!("unreachable");
}
