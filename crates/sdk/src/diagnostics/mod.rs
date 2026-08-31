#[cfg(not(target_arch = "riscv32"))]
pub use ::thiserror::Error;

#[cfg(not(target_arch = "riscv32"))]
mod debug_context;

#[cfg(not(target_arch = "riscv32"))]
pub use debug_context::*;

#[cfg(not(target_arch = "riscv32"))]
#[macro_export]
macro_rules! with_debug_context {
    { $call_info:expr, $body:block } => {
        $crate::with_debug_context(&$call_info, || $body)
    };
}

#[cfg(target_arch = "riscv32")]
// Omit context tracking
#[macro_export]
macro_rules! with_debug_context {
    { $call_info:expr, $body:block } => {
        $body
    };
}

#[cfg(not(target_arch = "riscv32"))]
#[macro_export]
macro_rules! with_call_tracking {
    { $call_info:expr, $body:expr } => {
        ::sdk::with_debug_context(&$call_info, || { $body })
    };
}

#[cfg(target_arch = "riscv32")]
// Omit call-tracking
#[macro_export]
macro_rules! with_call_tracking {
    { $call_info:expr, $body:expr } => {
        $body
    };
}
