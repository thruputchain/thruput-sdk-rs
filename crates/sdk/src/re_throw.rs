/// A type that can re-throw itself as a contract exception or fault.
pub trait ReThrow {
    /// Re-throw this error. Exceptions are propagated as exceptions;
    /// faults panic (a fault stays a fault).
    fn re_throw(self) -> !;
}

/// Extension trait for transparently propagating a contract error.
pub trait OrReThrow<T> {
    /// On `Err`, re-throw the error. On `Ok`, return the value.
    fn or_re_throw(self) -> T;
}

impl<T, E: ReThrow> OrReThrow<T> for Result<T, E> {
    #[cfg_attr(feature = "debugging", track_caller)]
    fn or_re_throw(self) -> T {
        match self {
            Ok(v) => v,
            Err(e) => e.re_throw(),
        }
    }
}
