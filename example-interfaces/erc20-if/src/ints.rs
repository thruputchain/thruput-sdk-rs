use sdk::prelude::*;

#[derive(ReadXDR)]
#[xdrs(as_type = "uint128_xdr")]
pub struct U128(u128);

impl From<uint128_xdr> for U128 {
    fn from(_u: uint128_xdr) -> Self {
        Self(0)
    }
}

#[derive(ReadXDR)]
#[xdrs(as_type = "int128_xdr")]
pub struct I128(i128);

impl From<int128_xdr> for I128 {
    fn from(_i: int128_xdr) -> Self {
        Self(0)
    }
}
