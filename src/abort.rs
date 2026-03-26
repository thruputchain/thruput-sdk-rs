#[cfg(feature = "debugging")]
pub use ::abort::with_debug_context;

/// Abort the contract with a contract-specific error.
#[cfg_attr(feature = "debugging", track_caller)]
pub fn abort(err: &impl crate::exn::Exn) -> ! {
    crate::exn::cross_exn::serialize_error(err)
}
