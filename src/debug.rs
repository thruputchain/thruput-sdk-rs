#[cfg(feature = "call_tracking")]
#[macro_export]
macro_rules! with_debug_context {
    { $call_info:expr, $body:block } => {
        $crate::with_debug_context(&$call_info, || $body)
    };
}

#[cfg(not(feature = "call_tracking"))]
// Omit tracking
#[macro_export]
macro_rules! with_debug_context {
    { $call_info:expr, $body:block } => {
        $body
    };
}
