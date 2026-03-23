use ::abort::{AbortContext, AbortHandler, AbortPayload};
use core::convert::Infallible;

pub use ::abort::{AbortInfo, Never};

#[cfg(feature = "debugging")]
pub use ::abort::with_debug_context;

::abort::abort_context! {
     pub static CONTRACT_ABORT_CONTEXT: AbortContext = {
         handler: &contract_abort_handler,
     }
}

#[cfg(feature = "debugging")]
pub fn contract_abort_handler<'a>(info: AbortInfo<'a>) -> Never {
    let a = crate::CrossExn::<Infallible>::from_abort_info(info);
    crate::contract_abort_encoded(&a);
}

#[cfg(not(feature = "debugging"))]
pub fn contract_abort_handler<'a>(info: AbortInfo<'a>) -> Never {
    crate::CrossExn::<Infallible>::serialize_abort(info.payload());
}

fn with_contract_ctx<F, T>(f: F) -> T
where
    F: FnOnce(&AbortContext) -> T,
{
    ::abort::with_static_context(&CONTRACT_ABORT_CONTEXT, f)
}

/// Call the current abort handler with the provided payload.
///
/// _Panics_ if no abort-handler has been installed.
///
#[cfg_attr(feature = "debugging", track_caller)]
pub fn abort(payload: &dyn AbortPayload) -> ! {
    with_contract_ctx(|ctx| ctx.abort(payload))
}

/// Install the given abort handler while executing the provided function.
///
/// When this function returns, the abort handler is reset to its state before.
///
pub fn with_abort_handler<F, T>(handler: AbortHandler<'_>, thunk: F) -> T
where
    F: FnOnce() -> T,
{
    with_contract_ctx(|ctx| ctx.with_abort_handler(handler, thunk))
}

/// Install the given abort handler while executing the provided code.
///
/// When this function returns, the abort handler is reset to its state before.
///
#[macro_export]
macro_rules! with_abort_handler {
    ( $handler:expr, $body:expr $(,)? ) => {
        $crate::with_abort_handler(&$handler, || $body)
    };
}

/// Catch abort payloads of a particular type.
///
/// If the thunk aborts, pass the payload to the handler if it has type `E`,
/// else forward the abort info to the next outer handler.
///
pub fn catch<H, E, F, T>(handler: H, thunk: F) -> T
where
    H: Fn(&E) -> Never + 'static,
    E: 'static,
    F: FnOnce() -> T,
{
    with_contract_ctx(|ctx| ctx.catch(handler, thunk))
}
