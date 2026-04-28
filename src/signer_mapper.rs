use crate::xdr::*;

/// Marker trait for types that have an unsigned (pre-signing) representation.
///
/// This is the non-alloc portion of the Mintable system: it only declares
/// the associated `Unsigned` type, with no methods that require allocation.
/// The full [`Mintable`](crate::mintable::Mintable) trait (alloc-gated)
/// extends this with minting methods.
pub trait MintableType: Sized + WriteXDR + for<'r> ReadXDR<'r> {
    type Unsigned: WriteXDR + for<'r> ReadXDR<'r>;
}

pub trait SignerMapper {
    type Map<T: MintableType>: WriteXDR + for<'r> ReadXDR<'r>;
}

pub struct Signed;
impl SignerMapper for Signed {
    type Map<T: MintableType> = T;
}
