//! System functions: trap and panic

#[cfg(target_arch = "wasm32")]
#[inline(always)]
pub fn trap() -> ! {
    core::arch::wasm32::unreachable()
}

#[cfg(all(not(target_arch = "wasm32"), not(feature = "mock")))]
#[inline(always)]
pub fn trap() -> ! {
    panic!("unreachable");
}

#[cfg(not(feature = "std"))]
#[panic_handler]
fn handle_panic(_: &core::panic::PanicInfo) -> ! {
    trap();
}

#[cfg(feature = "mock")]
mod test_rt {
    use native_exec::invoke_error::deterministic_error;
    use std::boxed::Box;
    use std::panic::panic_any;

    pub fn trap() -> ! {
        panic_any(Box::new(deterministic_error()))
    }
}

#[cfg(feature = "mock")]
#[allow(dead_code)]
#[allow(unused)]
pub use test_rt::trap;

#[macro_export]
macro_rules! contract_panic {
    ( $($expr:expr),* ) => {
        {
            #[cfg(feature = "mock")]
            panic!($($expr),*);

            #[cfg(not(feature = "mock"))]
            {
                // Reference the expressions to avoid complaints
                // about unused variables.
                // TODO: does this actually compile away?
                let _ = format_args!($($expr),*);

                $crate::rt::trap();
            }
        }
    };
}
