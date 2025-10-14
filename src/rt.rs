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
    ($fmt:expr $(, $expr:tt),* ) => {
        {
            // TODO: confirm that this distinction is no longer necessary because we are using
            // `panic_immediate_abort`

            #[cfg(feature = "mock")]
            panic!($fmt $(, $expr)*);

            #[cfg(all(not(feature = "mock"), not(feature = "logging")))]
            {
                // "use" any variables in the panic args
                let _ = ($($expr),*);
                let _ = $fmt;

                $crate::rt::trap();
            }

            #[cfg(all(not(feature = "mock"), feature = "logging"))]
            {
                $crate::log(alloc::format!($fmt $(, $expr)*).as_bytes()).unwrap();
                $crate::rt::trap();
            }
        }
    };
}
