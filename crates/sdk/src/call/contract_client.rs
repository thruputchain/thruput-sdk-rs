pub trait ContractClient {
    /// The id of the interface this client calls, matching that interface's
    /// `INTERFACE_ID`.
    const INTERFACE_ID: crate::InterfaceId;

    fn new(address: &crate::types::ContractAddress) -> Self;
}

/// Classify exceptions before the call scratch is released.
///
/// Generated clients implement this trait. Use
/// `client.map_exceptions(classify).try_method(...)` to return a compact
/// exception value instead of an owned [`RawException`](crate::exn::RawException).
/// The adapter preserves the client's address, gas limit and invocation flags,
/// and is consumed by one call. Only a parsed exception invokes the mapper;
/// success, faults, rejected calls and encode/decode errors keep their usual
/// [`RawCallError`](super::RawCallError) representation.
///
/// The mapper borrows the abort bytes. It may decode a small error, rethrow
/// verbatim, or explicitly take an owned copy. Keep it small: the call scratch
/// remains live until it returns. Do other SDK work or throw a new typed error
/// after the mapped call returns. A borrowed payload cannot escape the mapper.
///
/// Import this trait (included in `use thruput_sdk::*`) for method syntax.
/// If a contract has its own `map_exceptions` method, use
/// `MapExceptions::map_exceptions(&client, classify)` instead.
pub trait MapExceptions {
    /// A generated client's single-call adapter with mapper `F`.
    type Mapped<F>;

    /// Map a borrowed exception during the next generated `try_*` call.
    fn map_exceptions<X, F>(&self, on_exn: F) -> Self::Mapped<F>
    where
        F: for<'s> FnOnce(crate::exn::RawExceptionRef<'s>) -> X;
}
