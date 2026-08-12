//! XDR serialization and contract-spec integration.
//!
//! Wire format is eight `u32` limbs **most significant first**, matching the
//! `(high, low)` convention xdrs uses for `u128` -- so the encoded form is the
//! value as 32 big-endian bytes, the reverse of the in-memory order.
//!
//! `CollectAuthHandles`, `ReceiveAuthHandles` and `MintableType` are owned by
//! `sdk` and implemented there, for this type.

use crate::U256;
use groundhog_contract_spec as spec;
use xdrs::{
    ReadResult, ReadXDR, Reader, WriteResult, WriteXDR, Writer,
    WriterUnchecked,
};

// ---------------------------------------------------------------------------
// XDR
// ---------------------------------------------------------------------------

// Eight `u32` limbs most-significant-first is byte-for-byte a fixed opaque of
// 32 big-endian bytes, which is what `to_be_bytes` already produces -- and
// what xdrs encodes a `[u8; 32]` as. So these delegate rather than looping
// over limbs in reverse. `#[derive(ReadXDR, WriteXDR)]` cannot express it:
// it walks fields in declaration order, and the in-memory order is the
// opposite one.
impl<'r> ReadXDR<'r> for U256 {
    #[inline]
    fn read_xdr<R: Reader<'r>>(r: &mut R) -> ReadResult<Self> {
        Ok(Self::from_be_bytes(<[u8; 32] as ReadXDR>::read_xdr(r)?))
    }
}

impl WriteXDR for U256 {
    #[inline]
    fn write_xdr<F, W: Writer<F>>(&self, w: &mut W) -> WriteResult<()> {
        self.to_be_bytes().write_xdr(w)
    }

    #[inline]
    unsafe fn write_xdr_unchecked<F, W: WriterUnchecked<F>>(&self, w: &mut W) {
        // SAFETY: caller guarantees space for 32 bytes; a fixed opaque has no
        // XDR bounds to violate.
        unsafe { self.to_be_bytes().write_xdr_unchecked(w) }
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
