#[cfg(feature = "debugging")]
pub use ::thiserror::Error;

#[cfg(feature = "debugging")]
mod debug_context;

#[cfg(feature = "debugging")]
pub use debug_context::*;

#[cfg(feature = "debugging")]
#[macro_export]
macro_rules! with_debug_context {
    { $call_info:expr, $body:block } => {
        $crate::with_debug_context(&$call_info, || $body)
    };
}

#[cfg(not(feature = "debugging"))]
// Omit context tracking
#[macro_export]
macro_rules! with_debug_context {
    { $call_info:expr, $body:block } => {
        $body
    };
}

#[cfg(feature = "call_tracking")]
#[macro_export]
macro_rules! with_call_tracking {
    { $call_info:expr, $body:expr } => {
        ::sdk::with_debug_context(&$call_info, || { $body })
    };
}

#[cfg(not(feature = "call_tracking"))]
// Omit call-tracking
#[macro_export]
macro_rules! with_call_tracking {
    { $call_info:expr, $body:expr } => {
        $body
    };
}
