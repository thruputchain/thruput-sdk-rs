//! XDR serialization and contract-spec integration.
//!
//! Wire format is eight `u32` limbs **most significant first**, matching the
//! `(high, low)` convention xdrs uses for `u128` -- so the encoded form is the
//! value as 32 big-endian bytes, the reverse of the in-memory order. [`I256`]
//! rides the same encoding over its two's complement bits, which is what
//! Ethereum's `int256` uses and what any non-Rust SDK would guess.
//!
//! `CollectAuthHandles`, `ReceiveAuthHandles` and `MintableType` are owned by
//! `thruput-sdk` and implemented there, for this type.

use crate::{I256, U256};
use thruput_contract_spec as spec;
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

impl<'r> ReadXDR<'r> for I256 {
    #[inline]
    fn read_xdr<R: Reader<'r>>(r: &mut R) -> ReadResult<Self> {
        Ok(Self::from_be_bytes(<[u8; 32] as ReadXDR>::read_xdr(r)?))
    }
}

impl WriteXDR for I256 {
    #[inline]
    fn write_xdr<F, W: Writer<F>>(&self, w: &mut W) -> WriteResult<()> {
        self.to_be_bytes().write_xdr(w)
    }

    #[inline]
    unsafe fn write_xdr_unchecked<F, W: WriterUnchecked<F>>(&self, w: &mut W) {
        // SAFETY: as above.
        unsafe { self.to_be_bytes().write_xdr_unchecked(w) }
    }
}

// ---------------------------------------------------------------------------
// Contract spec
// ---------------------------------------------------------------------------

impl spec::ContractTypeRefSpec for U256 {
    const INFO: spec::ContractTypeInfo<'static> = spec::ContractTypeInfo {
        ty: spec::ContractTypeView::U256,
    };
}

impl spec::ContractTypeRefSpec for I256 {
    const INFO: spec::ContractTypeInfo<'static> = spec::ContractTypeInfo {
        ty: spec::ContractTypeView::I256,
    };
}
