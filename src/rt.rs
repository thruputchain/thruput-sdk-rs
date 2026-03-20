//! Runtime exit (trap)

#[cfg(not(feature = "mock"))]
#[macro_export]
/// Halt execution
macro_rules! fault {
    { $( $expr:expr ),* } => {
        {
            let _ = ( $($expr),* );

            $crate::trap();
        }
    };
}

#[cfg(feature = "mock")]
#[macro_export]
/// Halt execution
macro_rules! fault {
    { $( $expr:expr ),* } => {
        {
            // XXX: This may be a recursive panic
            ::std::panic!( $($expr),* );
        }
    };
}

#[cfg(target_arch = "wasm32")]
#[inline(always)]
/// WASM: execute an illegal instruction
pub fn trap() -> ! {
    core::arch::wasm32::unreachable()
}

#[cfg(feature = "mock")]
#[allow(dead_code)]
#[allow(unused)]
/// Toss a deterministic error for the mock execution environment to catch
pub fn trap() -> ! {
    use groundhog_native_exec::invoke_error::deterministic_error;
    use std::panic::panic_any;

    panic_any(deterministic_error())
}

#[cfg(all(not(target_arch = "wasm32"), not(feature = "mock")))]
#[inline(always)]
pub fn trap() -> ! {
    panic!("unreachable");
}
