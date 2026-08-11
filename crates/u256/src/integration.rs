//! XDR serialization and contract-spec integration.
//!
//! Wire format is eight `u32` limbs **most significant first**, matching the
//! `(high, low)` convention xdrs uses for `u128` -- so the encoded form is the
//! value as 32 big-endian bytes. That is the reverse of the in-memory order,
//! which is why these are hand-written rather than derived.
//!
//! `CollectAuthHandles`, `ReceiveAuthHandles` and `MintableType` are owned by
//! `sdk` and implemented there, for this type.

use crate::{LIMBS, U256};
use groundhog_contract_spec as spec;
use xdrs::{
    ReadResult, ReadXDR, Reader, WriteResult, WriteXDR, Writer,
    WriterUnchecked,
};

// ---------------------------------------------------------------------------
// XDR
// ---------------------------------------------------------------------------

impl<'r> ReadXDR<'r> for U256 {
    #[inline]
    fn read_xdr<R: Reader<'r>>(r: &mut R) -> ReadResult<Self> {
        let mut limbs = [0u32; LIMBS];
        // Most significant first.
        let mut i = LIMBS;
        while i > 0 {
            i -= 1;
            limbs[i] = <u32 as ReadXDR>::read_xdr(r)?;
        }
        Ok(Self::from_limbs(limbs))
    }
}

impl WriteXDR for U256 {
    #[inline]
    fn write_xdr<F, W: Writer<F>>(&self, w: &mut W) -> WriteResult<()> {
        let mut i = LIMBS;
        while i > 0 {
            i -= 1;
            <u32 as WriteXDR>::write_xdr(&self.limbs[i], w)?;
        }
        Ok(())
    }

    #[inline]
    unsafe fn write_xdr_unchecked<F, W: WriterUnchecked<F>>(&self, w: &mut W) {
        let mut i = LIMBS;
        while i > 0 {
            i -= 1;
            // SAFETY: the caller guarantees space for all eight limbs, and a
            // `u32` has no XDR bounds to violate.
            unsafe {
                <u32 as WriteXDR>::write_xdr_unchecked(&self.limbs[i], w);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Contract spec
// ---------------------------------------------------------------------------

// The spec impls, anchor fn and raw link-section fragment come from
// `#[derive(ContractTypeSpec)]` in `lib.rs`. `ContractTypeName` is the
// exception -- the derive references it but never emits it.
impl spec::ContractTypeName for U256 {
    const NAME: &'static str = "U256";
}
