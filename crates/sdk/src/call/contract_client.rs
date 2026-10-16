pub trait ContractClient {
    /// The id of the interface this client calls, matching that interface's
    /// `INTERFACE_ID`.
    const INTERFACE_ID: crate::InterfaceId;

    fn new(address: &crate::types::ContractAddress) -> Self;
}

/// Classify a borrowed exception during a generated call.
///
/// Generated clients implement this trait. Use
/// `client.map_exceptions(classify).try_method(...)` to return a compact
/// exception value instead of an owned [`RawException`](crate::exn::RawException).
/// The adapter preserves the client's address, gas limit and invocation flags,
/// and is consumed by one call. Only a parsed exception invokes the mapper;
/// success, faults, rejected calls and encode/decode errors keep their usual
/// [`RawCallError`](super::RawCallError) representation.
///
/// The mapper borrows abort bytes already copied from the host into guest
/// scratch; its return value cannot borrow them. This avoids an additional
/// owned exception buffer, not the host/guest copies.
/// Prefer a compact classification or verbatim rethrow. Taking an owned copy
/// is allowed, but in RV32 contracts with `alloc` that allocation is not
/// reclaimed during the invocation. Decoding types with owned fields can
/// also allocate.
///
/// The call scratch stays checked out during the mapper. Do other SDK work
/// or throw a new typed error after the mapped call returns, when its scratch
/// checkout has ended. Without `alloc`, this does not guarantee that
/// the compiler reuses its stack slot; test the release RV32 artifact.
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
