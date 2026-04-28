use crate::xdr::*;

/// Marker trait for types that have a pre-signing (preimage) representation.
///
/// `Preimage` is intentionally unbound — the `WriteXDR + ReadXDR` requirement
/// is deferred to the `Mintable` trait so that recursive contracttypes can
/// implement `MintableType` without triggering trait-solver overflow.
pub trait MintableType: Sized + WriteXDR + for<'r> ReadXDR<'r> {
    type Preimage;
}
