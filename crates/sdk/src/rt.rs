//! Runtime exit (trap)

#[cfg(feature = "dev")]
#[allow(dead_code)]
#[allow(unused)]
/// Toss a deterministic error for the mock execution environment to catch
pub fn trap() -> ! {
    use groundhog_native_exec::invoke_error::deterministic_error;
    use std::panic::panic_any;

    panic_any(deterministic_error())
}

#[cfg(not(feature = "dev"))]
#[inline(always)]
pub fn trap() -> ! {
    panic!("unreachable");
}
